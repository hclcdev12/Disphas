# disphas — Protocole UI ↔ helper root

> Statut : **v0.1 — décisions validées le 2026-10-03** (lancement par session via pkexec, JSON par ligne,
> tâche menée à terme si l'UI se ferme). Crate : `disphas-protocol`.

## 1. Rôle et menaces

Ce protocole est **la frontière de sécurité** de disphas. Il est conçu comme si l'UI pouvait être boguée ou compromise.

| Risque | Parade |
|---|---|
| Effacer le mauvais disque (`/dev/sdX` qui change entre inspection et effacement) | Le helper travaille sur une **identité** (S/N + WWN/EUI-64/NGUID), relue juste avant l'exécution. Le chemin n'est qu'un indice. |
| UI qui demande une méthode interdite ou contourne un Say NO | Le **moteur de décision tourne dans le helper**. Un `Sanitize` doit citer un `verdict_id` émis par le helper ; toute divergence est refusée. |
| Autre processus qui parle au helper | Canal = **pipe privé** stdin/stdout détenu par le seul processus parent. Aucun socket. |
| Trop de code root | Le helper ne fait que : disques, décision, signature. Rendu PDF et traductions restent côté utilisateur. |

## 2. Couches

```
React (WebView)  ──tauri::command──►  disphas-app (Rust, utilisateur)  ──pipe JSON──►  disphas-helper (root)
                 ◄──événements Tauri──                                 ◄──────────────
```

React ne parle **jamais** directement au helper. `disphas-app` relaie, traduit, rend le PDF.
Les types TypeScript sont générés depuis les types Rust : une seule source de vérité.
Outil retenu le 2026-10-04 : **`ts-rs`** (stable, comprend les attributs serde) plutôt que `specta` / `tauri-specta`
(encore en version candidate). Génération : `npm run bindings` dans `app/`, vers `app/src/bindings/`.
Les entiers 64 bits sont exportés en `number` (toutes les valeurs du protocole restent sous 2⁵³).

## 3. Cycle de vie

1. Au démarrage, `disphas-app` lance `pkexec /usr/libexec/disphas/disphas-helper`.
   Une action polkit dédiée (`org.disphas.helper`, exigence `auth_admin`) affiche un message clair à l'utilisateur.
2. Le helper vérifie qu'il est bien lancé par pkexec (`PKEXEC_UID`), purge son environnement, puis attend `Hello`.
   *Implémenté le 2026-10-04 (builds release)* : refus de démarrer si l'UID effectif (lu dans `/proc/self/status`)
   n'est pas 0 ou si `PKEXEC_UID` est absent. **Écart** : l'environnement n'est pas réécrit
   (`remove_var` est `unsafe` en Rust 2024 et le helper interdit l'unsafe) ; garantie équivalente : un helper
   release ne lit aucune variable d'environnement hormis `PKEXEC_UID` et ne lance aucun processus ;
   pkexec nettoie déjà l'environnement.
3. Le helper vit tant que l'UI est ouverte.
4. **Fermeture de l'UI** (fin de stdin) :
   - aucune tâche en cours → le helper s'arrête immédiatement ;
   - tâche en cours → elle va **à son terme** (exécution, vérification, validation, signature),
     le résultat est rangé dans `/var/lib/disphas/reports/`, puis le helper s'arrête.
5. Au prochain lancement, l'UI retrouve les résultats via `ListJobs` ou en lisant `/var/lib/disphas/reports/`.

## 4. Transport

- **JSON UTF-8, un message par ligne** (NDJSON) sur stdin/stdout.
- Taille maximale d'un message : **1 Mio** ; au-delà, le helper rejette et ferme.
- **File de sortie bornée** (256 messages) : si l'UI ne lit plus, le helper cesse de lire les requêtes
  jusqu'à ce qu'elle lise ses réponses ; les **événements** de tâche qui ne trouvent pas de place sont
  abandonnés (une tâche n'attend jamais l'UI), l'état reste disponible par `ListJobs`. Le détail d'erreur
  d'une requête rejetée, qui peut citer l'entrée, est tronqué à 256 octets.
- **stdout du helper est réservé au protocole.** Ses logs de diagnostic vont sur stderr, que l'app capture.
- Trois types de messages :

```jsonc
// Requête (UI → helper)
{"type":"request","id":42,"cmd":"Inspect","args":{"device":{…}}}

// Réponse (helper → UI), exactement une par requête
{"type":"response","id":42,"ok":true,"result":{…}}
{"type":"response","id":42,"ok":false,"error":{"code":"DEVICE_CHANGED","detail":"…"}}

// Événement (helper → UI), spontané
{"type":"event","event":"JobProgress","data":{…}}
```

## 5. Identité d'un disque

```jsonc
"device": {
  "node_hint": "/dev/nvme0n1",          // indice seulement, jamais utilisé tel quel
  "identity": {
    "model": "Samsung SSD 980 PRO 1TB",
    "serial": "S5GXNX0T1234567",
    "wwn": null, "eui64": "002538…", "nguid": null
  }
}
```

Avant toute action, le helper retrouve le disque **par son identité**. Si l'identité ne correspond plus → `DEVICE_CHANGED`.

## 6. Commandes (liste fermée)

| Commande | Arguments | Résultat |
|---|---|---|
| `Hello` | version du protocole, version de l'app | version du helper, `rules_version`, SHA-256 du binaire, état de licence. Versions incompatibles → `PROTOCOL_MISMATCH`. Version **2** depuis le 2026-10-20 (droits par offre, `PARALLEL_LIMIT`), **3** depuis le 2026-10-26 (`confirmation` facultative, contrôle 4) : l'app et le helper d'un même paquet parlent la même version |
| `ListDevices` | — | Disques avec identité, taille, type et état rapide (PROTÉGÉ, etc.) |
| `Inspect` | `device` | Profil complet + **verdict** + `verdict_id` (valable 10 min) + méthodes autorisées |
| `Unlock` | `device`, mot de passe ATA | Déverrouillage d'un disque *Locked* (état BLOQUÉ B2). Le mot de passe n'est jamais journalisé et est effacé de la mémoire après usage |
| `Sanitize` | voir §7 | `job_id` |
| `Cancel` | `job_id` | OK, ou `NOT_CANCELLABLE` pendant une phase firmware |
| `ListJobs` | — | Tâches en cours et terminées non encore récupérées |
| `DeleteReports` | `ids` (≤ 5000 identifiants de rapport) | Supprime des rapports stockés (durée de conservation du client, RGPD art. 5(1)(e)) ; renvoie le nombre supprimé. Chaque suppression est consignée (identifiant et date, sans donnée personnelle) dans `reports-deletions.log`, réservé à root. Ajouté le 2026-10-04 |
| `GetJobResult` | `job_id` | Statut final, attestation COSE (base64), journal JSON |
| `LicenseStatus` | — | Offre, validité, identifiant ; tant que la licence est valide, **droits** de l'offre décidés par le helper : `entitlements: { max_parallel, customer_logo }`, `max_parallel` `null` = sans limite ([offres](offres.md) §4.6, 2026-10-20). Même contenu dans `Hello` et en réponse à `InstallLicense` |
| `CreateActivationRequest` | — | Demande d'activation (contient la clé publique du poste) |
| `InstallLicense` | contenu du `.lic` | Nouvel état de licence, ou `LICENSE_INVALID` |
| `Shutdown` | — | Arrêt propre (refusé si une tâche est en cours) |

La validation et la signature **ne sont pas** des commandes : elles font partie d'un `Sanitize`.
Il est donc impossible de faire signer quelque chose qui n'a pas été effacé.

Le déblocage d'un disque *Frozen* (B1) passe par une mise en veille, déclenchée par l'app via logind (sans root).

## 7. `Sanitize` en détail

```jsonc
{"type":"request","id":7,"cmd":"Sanitize","args":{
  "verdict_id": "b2f1…",
  "device": { "node_hint": "/dev/nvme0n1", "identity": { … } },
  "method": null,                          // null = méthode recommandée ; sinon une des méthodes autorisées
  "confirmation": null,                    // mode strict : S/N lu sur le disque (entier, ou ses 4 derniers caractères) ; sinon null
  "acknowledged": ["D4", "R4", "NS_ALL"],  // avertissements dont l'opérateur a pris acte
  "force": null,                           // ou {"justification": "…"} — uniquement sur SAY NO
  "operator": "J. Martin", "organization": "Atelier XYZ"   // déclaratifs, inscrits tels quels
}}
```

Le helper contrôle, **dans cet ordre**, et refuse au premier échec :

1. `verdict_id` connu et non expiré → sinon `VERDICT_EXPIRED`.
2. Identité relue sur le disque = identité du verdict → sinon `DEVICE_CHANGED`.
3. Protections réévaluées (montages, swap, volumes…) → sinon `PROTECTED`.
4. Un disque sans S/N (vide ou blanc) n'est jamais effacé, et une `confirmation` présente (mode strict) vaut le
   S/N entier ou ses 4 derniers caractères → sinon `CONFIRMATION_MISMATCH`. `confirmation` absente : l'app a
   appliqué son délai d'annulation ([cadre](cadre_projet.md) §5.4, protocole 3, 2026-10-26).
5. `acknowledged` couvre **tous** les avertissements du verdict → sinon `ACK_MISSING`.
6. `method` ∈ méthodes autorisées par le verdict → sinon `VERDICT_MISMATCH`.
7. `force` présent seulement si verdict SAY NO, justification ≥ 20 caractères → sinon `FORCE_NOT_ALLOWED`.
8. Licence autorisant l'effacement → sinon `LICENSE_REQUIRED`.
9. Aucune autre tâche en cours **sur ce disque** → sinon `BUSY`. Les autres disques s'effacent **en parallèle**
   (exigé par le [cadre](cadre_projet.md) §1 ; décidé le 2026-10-04, remplace « une seule à la fois »).
   Le disque est reconnu par son numéro de série ; pour un NVMe c'est celui du **contrôleur**, commun à tous
   ses espaces de noms, si bien qu'un effacement de tous les espaces de noms ne croise jamais une autre tâche
   sur le même contrôleur.
10. Pas plus d'effacements en cours **en même temps** que l'offre de la licence n'en permet → sinon
    `PARALLEL_LIMIT` ([offres](offres.md) §4.4, ajouté le 2026-10-20). Les tâches reprises après un plantage
    comptent ; une tâche terminée (succès, échec, REFUS) libère sa place. Rien n'est lancé et le verdict
    **reste valable** : l'opérateur relance quand une place se libère. Aucun paramètre de la commande ne
    modifie ce contrôle. La limite vaut pour **tout le poste** (lot 2 bis, 2026-10-20) : le helper compte aussi
    les tâches des autres helpers (verrous de `jobs/`, §10), sous le verrou `jobs/start.guard` tenu jusqu'à ce
    que la nouvelle tâche soit visible des autres ; l'application refuse en outre une seconde instance.

`NS_ALL` : prise d'acte qu'un Sanitize NVMe, ou un Format NVM sur un contrôleur qui l'applique à tous (`FNA` bit 1), efface **tous** les namespaces du contrôleur.

## 8. Événements

| Événement | Contenu |
|---|---|
| `DeviceAdded` / `DeviceRemoved` | Identité, indice de chemin (surveillance udev) |
| `DeviceChanged` | Identité ; **invalide** les verdicts en cours sur ce disque |
| `JobPhase` | `Preparing` → `Canaries` → `Executing` → `Verifying` → `Validating` → `Signing` → `Done` ; ou `Interrupted` (tâche perdue dans un plantage, sans attestation, §10) |
| `JobProgress` | Pourcentage ; pour une réécriture : octets traités, débit, temps restant ; source (`firmware` / `host`). Pour ATA Security Erase, qui ne renvoie rien avant la fin : l'estimation du disque (IDENTIFY mots 89/90) en `remaining_seconds`, envoyée une fois au départ ; l'interface en tire un compte à rebours (affichage seulement, jamais certifié) |
| `JobLog` | Niveau + **code de message** + paramètres (traduit côté app, jamais de texte en dur) |
| `JobFinished` | `job_id`, statut final (CONFORME / NON CONFORME / REFUS) |

## 9. Codes d'erreur

`PROTOCOL_MISMATCH`, `BAD_REQUEST`, `UNKNOWN_COMMAND`, `DEVICE_NOT_FOUND`, `DEVICE_CHANGED`, `PROTECTED`,
`VERDICT_EXPIRED`, `VERDICT_MISMATCH`, `CONFIRMATION_MISMATCH`, `ACK_MISSING`, `FORCE_NOT_ALLOWED`,
`LICENSE_REQUIRED`, `LICENSE_INVALID` (licence refusée à l'installation, ajouté le 2026-10-04), `BUSY`, `NOT_CANCELLABLE`, `UNLOCK_FAILED`, `JOB_INTERRUPTED` (`GetJobResult` d'une tâche interrompue, ajouté le 2026-10-04), `DEVICE_IN_USE` (`Sanitize` : à
l'ouverture du disque, un autre programme l'utilise — monté, ouvert en exclusif, tenu ouvert par un processus ou un
namespace voisin, [sécurité](securite.md) S9 ; rien n'a été fait, l'opérateur ferme ce programme et relance ; ajouté le
2026-10-07 après le banc, où ce refus arrivait en `INTERNAL`), `PARALLEL_LIMIT` (`Sanitize`, contrôle 10 :
l'offre ne permet pas d'effacement de plus en même temps ; rien n'a été lancé, ajouté le 2026-10-20), `INTERNAL`. Un disque devenu protégé entre le verdict et
l'ouverture (protections revérifiées à l'ouverture) est refusé en `PROTECTED`, plus en `INTERNAL` (2026-10-07).

## 10. Persistance et reprise

```
/var/lib/disphas/
├── keys/      clé privée du poste (0600, root)
├── license/   .lic installé
├── jobs/      état de chaque tâche, écrit à chaque changement de phase
└── reports/   <id>.cose + <id>.json — historique, root seul, lu par le helper
```

- `reports/` sert aussi d'**historique**, lu **par le helper** (décidé le 2026-10-06, banc de test : un groupe
  d'accès obligeait à ajouter chaque opérateur puis à rouvrir sa session, ce qui contredit la simplicité visée).
  Commandes en lecture seule, sans licence requise :
  - `ListReports {before?}` → `{reports: [{id, attestation?, journal_sha256?}], more}` : du plus récent au plus
    ancien, pages de 200 ; `attestation` (base64) et `journal_sha256` (hex, calculé par le helper) absents si le
    rapport est illisible, trop gros (64 Kio d'attestation, 512 Kio de journal) ou un lien symbolique ;
  - `GetReport {id}` → `{id, attestation, journal}` : pour rendre ou réimprimer un certificat, ou un PDF de lot.
  L'app vérifie **chaque** rapport elle-même (signature, chaîne de licence vers une racine de confiance, empreinte du
  journal, identifiant = nom du fichier) avant de l'afficher ; un rapport qui échoue n'est montré que par son
  identifiant, jamais par son contenu. 5 000 rapports les plus récents au plus. Le dossier d'état est défini une
  seule fois, dans `disphas_license::state_dir()`.
  Arborescence créée par le paquet, **root seul** (dossier et sous-dossiers 0700, certificats 0600, jamais écrasés) ;
  le helper fixe `umask 077` et refuse de démarrer si le dossier d'état ou un sous-dossier n'est pas à lui, est un lien
  ou est ouvert en écriture aux autres ([sécurité](securite.md) S1). Le contrôle d'accès aux certificats est celui du
  helper : l'authentification administrateur polkit exigée pour lancer disphas ([conformité](conformite.md) R1).
- Si le helper plante en cours de tâche, le lancement suivant reprend depuis `jobs/` :
  - **Sanitize firmware** : lecture du journal de statut du disque, puis vérification, validation et signature ;
  - **réécriture hôte** : la tâche est marquée *interrompue*, sans attestation ; l'opérateur relance (nouvelle inspection).

**Implémentation** (2026-10-04) :

- `jobs/<id>.json` (0600, dossier 0700) contient tout ce qu'il faut pour finir la tâche : identité, profil
  et plan (verdict READY ou forçage) tels qu'à l'inspection, `rules_version`, graine et blocs échantillons,
  tentatives déjà faites, chronologie. Il est écrit **avant** toute écriture sur le disque (sinon la tâche ne
  démarre pas), réécrit de façon atomique à chaque phase et après chaque tentative, supprimé une fois
  l'attestation stockée.
- `jobs/<id>.lock` est verrouillé (`flock`) par le processus qui exécute la tâche : un second helper lancé
  en parallèle ne reprend ni n'écarte une tâche vivante. Les accès disque de la reprise (`O_EXCL`) le
  confirment. Ces verrous servent aussi à compter les effacements des autres helpers (contrôle 10, §7).
- `jobs/start.guard` (2026-10-20) : verrou commun à tous les helpers du poste, pris (en attente si besoin)
  pendant le contrôle 10 et jusqu'à ce que le verrou de la nouvelle tâche existe (30 s au plus), et pendant la
  reprise des tâches laissées. Deux helpers ne prennent donc jamais la dernière place en même temps, et aucun
  ne teste le verrou d'une tâche qu'un autre est en train de reprendre. Le fichier reste en place.
- La reprise a lieu à la première commande qui suit un `Hello`, pour que l'app connaisse déjà le helper
  quand arrivent les événements.
- **Reprise** seulement si : méthode **Sanitize** (NVMe ou ATA), phase atteinte `Executing` ou au-delà, même
  `rules_version`. Le helper n'envoie **pas** de nouvelle commande : il lit le journal de statut jusqu'à la
  fin, puis valide par les canaris d'origine, évalue et signe (CONFORME seulement si les canaris ont disparu ;
  sinon nouvelle tentative ou REFUS, comme d'habitude). Le journal porte un événement `resumed`.
- **Interrompue** (`JobPhase::Interrupted`, `GetJobResult` → `JOB_INTERRUPTED`), fichiers supprimés : écriture
  hôte, Format NVM et Security Erase (aucun état fiable après une coupure ; un mot de passe Security Erase
  resté sur le disque est retiré par la reprise de `moteur_decision.md` §4), tâche arrêtée avant `Executing`,
  autre `rules_version`. Elle n'apparaît que dans la session qui suit le plantage.
- Disque absent, licence invalide ou disque occupé : la tâche est **gardée** pour un lancement ultérieur.

## 11. Règles d'implémentation du helper

- Aucune commande shell, aucun chemin fourni par l'UI utilisé tel quel.
- Toute entrée bornée en taille et validée avant usage ; désérialisation stricte (`deny_unknown_fields`).
- Secrets (mot de passe ATA, clé privée) effacés de la mémoire après usage (`zeroize`), jamais journalisés.
- Fuzzing de la désérialisation (`cargo-fuzz`) dans la CI. Implémenté le 2026-10-04 (`fuzz/`, voir son README) :
  7 cibles couvrant toutes les entrées non fiables (protocole dans les deux sens, session complète du helper,
  attestations, QR, listes de révocation, PDF, licences et demandes d'activation, données renvoyées par les disques).
  Premier passage de 60 s par cible : aucun plantage. ⚠️ Pas encore de CI : à brancher quand elle existera.
- Fixtures JSON de référence partagées entre tests Rust et TypeScript.

## 12. Exemple de séquence complète

```
UI                                   helper
 │── Hello ─────────────────────────►│
 │◄──────────────── ok (versions) ───│
 │── ListDevices ───────────────────►│
 │◄────────────────── ok (2 disques)─│
 │── Inspect(nvme0) ────────────────►│
 │◄─ ok (PRÊT, Purge, verdict_id) ───│
 │  [opérateur tape 4567, coche les avertissements]
 │── Sanitize(verdict_id, …) ───────►│
 │◄──────────────── ok (job_id) ─────│
 │◄─ event JobPhase Canaries ────────│
 │◄─ event JobPhase Executing ───────│
 │◄─ event JobProgress 37 % ─────────│
 │◄─ event JobPhase Validating ──────│
 │◄─ event JobFinished CONFORME ─────│
 │── GetJobResult(job_id) ──────────►│
 │◄──── ok (attestation, journal) ───│
 │  [app rend le PDF dans la langue choisie]
```

## 13. Implémentation (`disphas-protocol`, décidé le 2026-10-04)

- **Indépendance du moteur** : `disphas-protocol` ne dépend pas de `disphas-core`. Il définit son propre format
  (verdict, codes, méthodes) ; le helper convertit par `match` exhaustif, si bien qu'un code ajouté au moteur
  sans équivalent dans le protocole casse la compilation. L'app n'embarque donc jamais le moteur de décision.
- **Décodage en deux temps** : l'enveloppe (`type`, `id`, `cmd`, `args`) est lue strictement, puis `args` est
  décodé pour la commande nommée. Une requête rejetée reçoit sa réponse avec son `id` quand il est lisible ;
  `UNKNOWN_COMMAND` et `BAD_REQUEST` sont distingués. Message > 1 Mio : connexion fermée.
- Les commandes sans argument (`ListDevices`, `ListJobs`, `LicenseStatus`, `CreateActivationRequest`, `Shutdown`)
  s'envoient **sans** champ `args`.
- **Verdict sur le fil** (`Inspect`) : `{"state": "PROTECTED" | "BLOCKED" | "SAY_NO" | "READY", …}`.
  Chaque méthode autorisée est un `MethodChoice { method, category, reserves, all_namespaces }` ;
  `required_acknowledgements()` donne la liste exacte à cocher (codes D, réserves, `NS_ALL`), partagée par l'app et le helper.
  SAY NO porte `forcible` (méthode forçable ou `null`).
- **Statuts finaux** sur le fil : `CONFORME`, `NON_CONFORME`, `REFUS` (mêmes codes que l'attestation).
- **Mot de passe ATA** : 32 octets au plus, effacé de la mémoire (`zeroize`), jamais affiché par `Debug`.
  Le helper doit aussi effacer la ligne brute qui le contenait.
- **Limites connues** : `Inspect` ne transmet pas encore le profil S.M.A.R.T. complet ; les clés JSON dupliquées
  à l'intérieur de `args` ne sont pas rejetées (la dernière l'emporte) ; bornes de longueur par champ à ajouter
  côté helper lors de la validation métier.

### Helper (`disphas-helper`, squelette du 2026-10-04)

- Bibliothèque + binaire : session, boucle et conversion testables sans processus, sans root ni disque.
  Le binaire tourne sur `NoDevices` (aucun disque visible) tant que `disphas-device` n'est pas branché.
- Toute commande avant un `Hello` réussi → `BAD_REQUEST`.
- Ligne illisible **sans `id` exploitable** (JSON invalide, UTF-8 invalide) : la connexion est fermée,
  faute de pouvoir répondre à une requête précise. Avec un `id` : réponse d'erreur, la session continue.
- `Sanitize` : les contrôles du §7 sont implémentés dans l'ordre (9 au départ, 10 depuis le 2026-10-20). Précisions :
  - contrôle 2 : l'identité de la requête doit égaler celle du verdict, **et** le disque est relu par identité ;
  - contrôle 4 : comparaison **exacte** (casse comprise) des 4 derniers **caractères** du S/N ;
  - contrôle 5 : `detail` liste les codes manquants (`"D4,R7"`) pour que l'app les signale ;
  - contrôle 5 porte sur la méthode **choisie** (R7 exigé pour une écriture hôte en alternative) ;
  - `operator` (≤ 64 caractères) et `organization` (≤ 128), signés tels quels : espaces de bord retirés, vide =
    absent, caractères de contrôle refusés (`BAD_REQUEST`) — ajouté le 2026-10-04 ; caractères de mise en forme
    invisibles ou d'inversion de sens (U+202E, U+200B…) refusés aussi, comme dans le libellé de lot ; justification
    de forçage ≤ 500 caractères, retours à la ligne seuls permis (`FORCE_NOT_ALLOWED`) ;
  - au-delà des contrôles, l'exécution n'est pas encore implémentée (`INTERNAL`).
- `Unlock` (2026-10-04) : `kind` = `User` ou `Master` (le maître ne marche qu'en niveau de sécurité *High*) ; déverrouille (SECURITY UNLOCK, mot de passe utilisateur complété par des zéros
  jusqu'à 32 octets) puis **retire** le mot de passe (SECURITY DISABLE PASSWORD), pour que le disque
  devienne effaçable ; le disque est retrouvé par identité ; chaque échec compte dans le compteur du disque
  (B6) ; `UNLOCK_FAILED` sans jamais reprendre le mot de passe dans `detail`. Refusé pendant une tâche **sur ce disque**.
- `CreateActivationRequest`, `InstallLicense`, `Cancel`, `GetJobResult` : pas encore implémentés ;
  `binary_sha256` est vide et la licence est `Missing` (donc tout `Sanitize` s'arrête au contrôle 8).
- Non encore fait : purge de l'environnement (§3).

### Licence (branchée le 2026-10-04)

- États sur le fil : `Missing`, `Valid`, `Expired`, `Revoked`, plus `NotYetValid` et `Invalid`
  (fichier altéré, mauvaise racine ou mauvais poste), que la licence distinguait déjà.
- L'état est relu et revérifié à chaque `Hello`, `LicenseStatus` et contrôle 8 de `Sanitize`.
- **Racines de confiance** (`disphas_license::trusted_roots()`, 2026-10-04) : la **clé racine officielle**
  (`disphas_attestation::trust::OFFICIAL_ROOT_KEY`, générée hors ligne le 2026-10-04) ; en développement
  **en plus**, la clé racine de développement publique (`disphas_license::dev_root`), absente des binaires
  release (vérifié : 0 occurrence de sa graine). Une licence ou un certificat est accepté s'il remonte à l'une
  d'elles. `disphas-verify` n'utilise que la clé officielle, sauf `--root-key` explicite (avec avertissement).
- **Dossier d'état** : `/var/lib/disphas` en release (environnement ignoré) ; en développement,
  `DISPHAS_STATE_DIR` ou `~/.local/state/disphas-dev`.
- Licence de développement : `cargo run -p disphas-license --example issue_dev_license -- 'DSA1:…' [jours]`.
- **Licences en ligne** (décidé le 2026-10-11, non implémenté, [licences_en_ligne.md](licences_en_ligne.md)) :
  **aucune commande nouvelle**. L'application obtient la demande par `CreateActivationRequest`, l'échange elle-même
  avec le serveur de licences, puis installe la réponse par `InstallLicense`, vérifiée comme toute licence. Le
  helper n'ouvre jamais de connexion réseau.

### Exécution (moteur de tâches, 2026-10-04)

- Un `Sanitize` accepté consomme son `verdict_id` (usage unique) — ainsi que tout autre verdict du même disque
  (même n° de série), qui décrit un état que l'effacement va changer — et lance une tâche sur son propre fil ;
  le helper continue de répondre. Réponses et événements passent par un même fil d'écriture.
- Phases émises : `Preparing` → `Canaries` (méthodes firmware) → `Executing` → `Verifying` → `Validating`
  → `Signing` → `Done`, puis `JobFinished`. `JobProgress` : source `host` (octets, débit, temps restant)
  ou `firmware` (pourcentage). `JobLog` : codes `samples_selected`, `verification`, `validation`,
  `attempt_retry`, `job_failed`.
- Échantillons : bloc 0, bloc du milieu, dernier bloc + `validation_samples` blocs tirés de la graine.
  Canari = SHA-256(graine ‖ « canary » ‖ LBA) répété sur le bloc. Validation hôte : blocs à zéro.
- Toute tâche menée à son terme produit une attestation signée (CONFORME, NON CONFORME ou REFUS),
  rangée dans `reports/<id>.cose` et `reports/<id>.json`. L'identifiant de tâche est l'UUID v7 du rapport.
- Fermeture de l'UI pendant une tâche : le helper la termine avant de s'arrêter.
- `GetJobResult` sur une tâche en cours → `BUSY`. `Cancel` → `NOT_CANCELLABLE` en V1 : un effacement lancé
  va toujours à son terme.
- Non encore fait : persistance `jobs/` et reprise après plantage (§10).
- **Démo** (`--features demo`, debug uniquement) : trois disques fictifs adossés à des fichiers image de
  256 Mio dans `<état>/demo-images/` ; les méthodes firmware y sont **simulées** (journal :
  `simulated_firmware: true`), la réécriture hôte est réelle.

### Backends du helper (2026-10-04)

| Build | Disques vus |
|---|---|
| release | **réels** (`SystemBackend` : inventaire, inspection firmware en lecture seule, effacement) |
| debug | aucun (`NoDevices`) — **jamais** de disque réel par défaut |
| debug `--features demo` | disques fictifs sur fichiers image |
| debug `--features real-devices` | réels — banc de test uniquement, en root, avec accord du propriétaire |

Ouvrir un disque pour l'effacer refait un inventaire, refuse tout disque protégé, n'accepte qu'un périphérique
bloc et l'ouvre en `O_EXCL` (le noyau refuse un disque monté ou utilisé) et `O_DIRECT`. Inventaire impossible →
aucun disque listé.

