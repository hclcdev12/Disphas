# Oblivio — Protocole UI ↔ helper root

> Statut : **v0.1 — décisions validées le 2026-10-03** (lancement par session via pkexec, JSON par ligne,
> tâche menée à terme si l'UI se ferme). Crate : `oblivio-protocol`.

## 1. Rôle et menaces

Ce protocole est **la frontière de sécurité** d'Oblivio. Il est conçu comme si l'UI pouvait être boguée ou compromise.

| Risque | Parade |
|---|---|
| Effacer le mauvais disque (`/dev/sdX` qui change entre inspection et effacement) | Le helper travaille sur une **identité** (S/N + WWN/EUI-64/NGUID), relue juste avant l'exécution. Le chemin n'est qu'un indice. |
| UI qui demande une méthode interdite ou contourne un Say NO | Le **moteur de décision tourne dans le helper**. Un `Sanitize` doit citer un `verdict_id` émis par le helper ; toute divergence est refusée. |
| Autre processus qui parle au helper | Canal = **pipe privé** stdin/stdout détenu par le seul processus parent. Aucun socket. |
| Trop de code root | Le helper ne fait que : disques, décision, signature. Rendu PDF et traductions restent côté utilisateur. |

## 2. Couches

```
React (WebView)  ──tauri::command──►  oblivio-app (Rust, utilisateur)  ──pipe JSON──►  oblivio-helper (root)
                 ◄──événements Tauri──                                 ◄──────────────
```

React ne parle **jamais** directement au helper. `oblivio-app` relaie, traduit, rend le PDF.
Les types TypeScript sont générés depuis les types Rust : une seule source de vérité.
Outil retenu le 2026-10-04 : **`ts-rs`** (stable, comprend les attributs serde) plutôt que `specta` / `tauri-specta`
(encore en version candidate). Génération : `npm run bindings` dans `app/`, vers `app/src/bindings/`.
Les entiers 64 bits sont exportés en `number` (toutes les valeurs du protocole restent sous 2⁵³).

## 3. Cycle de vie

1. Au démarrage, `oblivio-app` lance `pkexec /usr/libexec/oblivio/oblivio-helper`.
   Une action polkit dédiée (`org.oblivio.helper`, exigence `auth_admin`) affiche un message clair à l'utilisateur.
2. Le helper vérifie qu'il est bien lancé par pkexec (`PKEXEC_UID`), purge son environnement, puis attend `Hello`.
3. Le helper vit tant que l'UI est ouverte.
4. **Fermeture de l'UI** (fin de stdin) :
   - aucune tâche en cours → le helper s'arrête immédiatement ;
   - tâche en cours → elle va **à son terme** (exécution, vérification, validation, signature),
     le résultat est rangé dans `/var/lib/oblivio/reports/`, puis le helper s'arrête.
5. Au prochain lancement, l'UI retrouve les résultats via `ListJobs` ou en lisant `/var/lib/oblivio/reports/`.

## 4. Transport

- **JSON UTF-8, un message par ligne** (NDJSON) sur stdin/stdout.
- Taille maximale d'un message : **1 Mio** ; au-delà, le helper rejette et ferme.
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
| `Hello` | version du protocole, version de l'app | version du helper, `rules_version`, SHA-256 du binaire, état de licence. Versions incompatibles → `PROTOCOL_MISMATCH` |
| `ListDevices` | — | Disques avec identité, taille, type et état rapide (PROTÉGÉ, etc.) |
| `Inspect` | `device` | Profil complet + **verdict** + `verdict_id` (valable 10 min) + méthodes autorisées |
| `Unlock` | `device`, mot de passe ATA | Déverrouillage d'un disque *Locked* (état BLOQUÉ B2). Le mot de passe n'est jamais journalisé et est effacé de la mémoire après usage |
| `Sanitize` | voir §7 | `job_id` |
| `Cancel` | `job_id` | OK, ou `NOT_CANCELLABLE` pendant une phase firmware |
| `ListJobs` | — | Tâches en cours et terminées non encore récupérées |
| `GetJobResult` | `job_id` | Statut final, attestation COSE (base64), journal JSON |
| `LicenseStatus` | — | Offre, validité, identifiant |
| `CreateActivationRequest` | — | Demande d'activation (contient la clé publique du poste) |
| `InstallLicense` | contenu du `.lic` | OK ou erreur de vérification |
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
  "confirmation": "4567",                  // 4 derniers caractères du S/N, saisis par l'opérateur
  "acknowledged": ["D4", "R4", "NS_ALL"],  // avertissements dont l'opérateur a pris acte
  "force": null,                           // ou {"justification": "…"} — uniquement sur SAY NO
  "operator": "J. Martin", "organization": "Atelier XYZ"   // déclaratifs, inscrits tels quels
}}
```

Le helper contrôle, **dans cet ordre**, et refuse au premier échec :

1. `verdict_id` connu et non expiré → sinon `VERDICT_EXPIRED`.
2. Identité relue sur le disque = identité du verdict → sinon `DEVICE_CHANGED`.
3. Protections réévaluées (montages, swap, volumes…) → sinon `PROTECTED`.
4. `confirmation` = 4 derniers caractères du S/N → sinon `CONFIRMATION_MISMATCH`.
5. `acknowledged` couvre **tous** les avertissements du verdict → sinon `ACK_MISSING`.
6. `method` ∈ méthodes autorisées par le verdict → sinon `VERDICT_MISMATCH`.
7. `force` présent seulement si verdict SAY NO, justification ≥ 20 caractères → sinon `FORCE_NOT_ALLOWED`.
8. Licence autorisant l'effacement → sinon `LICENSE_REQUIRED`.
9. Aucune autre tâche en cours (V1 : une seule à la fois) → sinon `BUSY`.

`NS_ALL` : prise d'acte qu'un Sanitize NVMe efface **tous** les namespaces du contrôleur.

## 8. Événements

| Événement | Contenu |
|---|---|
| `DeviceAdded` / `DeviceRemoved` | Identité, indice de chemin (surveillance udev) |
| `DeviceChanged` | Identité ; **invalide** les verdicts en cours sur ce disque |
| `JobPhase` | `Preparing` → `Canaries` → `Executing` → `Verifying` → `Validating` → `Signing` → `Done` |
| `JobProgress` | Pourcentage ; pour une réécriture : octets traités, débit, temps restant ; source (`firmware` / `host`) |
| `JobLog` | Niveau + **code de message** + paramètres (traduit côté app, jamais de texte en dur) |
| `JobFinished` | `job_id`, statut final (CONFORME / NON CONFORME / REFUS) |

## 9. Codes d'erreur

`PROTOCOL_MISMATCH`, `BAD_REQUEST`, `UNKNOWN_COMMAND`, `DEVICE_NOT_FOUND`, `DEVICE_CHANGED`, `PROTECTED`,
`VERDICT_EXPIRED`, `VERDICT_MISMATCH`, `CONFIRMATION_MISMATCH`, `ACK_MISSING`, `FORCE_NOT_ALLOWED`,
`LICENSE_REQUIRED`, `BUSY`, `NOT_CANCELLABLE`, `UNLOCK_FAILED`, `INTERNAL`.

## 10. Persistance et reprise

```
/var/lib/oblivio/
├── keys/      clé privée du poste (0600, root)
├── license/   .lic installé
├── jobs/      état de chaque tâche, écrit à chaque changement de phase
└── reports/   <id>.cose + <id>.json — historique, lisible par l'utilisateur
```

- `reports/` sert aussi d'**historique** : l'app peut le lire directement, sans passer par le helper.
- Si le helper plante en cours de tâche, le lancement suivant reprend depuis `jobs/` :
  - **Sanitize firmware** : lecture du journal de statut du disque, puis vérification, validation et signature ;
  - **réécriture hôte** : la tâche est marquée *interrompue*, sans attestation ; l'opérateur relance (nouvelle inspection).

## 11. Règles d'implémentation du helper

- Aucune commande shell, aucun chemin fourni par l'UI utilisé tel quel.
- Toute entrée bornée en taille et validée avant usage ; désérialisation stricte (`deny_unknown_fields`).
- Secrets (mot de passe ATA, clé privée) effacés de la mémoire après usage (`zeroize`), jamais journalisés.
- Fuzzing de la désérialisation (`cargo-fuzz`) dans la CI.
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

## 13. Implémentation (`oblivio-protocol`, décidé le 2026-10-04)

- **Indépendance du moteur** : `oblivio-protocol` ne dépend pas de `oblivio-core`. Il définit son propre format
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

### Helper (`oblivio-helper`, squelette du 2026-10-04)

- Bibliothèque + binaire : session, boucle et conversion testables sans processus, sans root ni disque.
  Le binaire tourne sur `NoDevices` (aucun disque visible) tant que `oblivio-device` n'est pas branché.
- Toute commande avant un `Hello` réussi → `BAD_REQUEST`.
- Ligne illisible **sans `id` exploitable** (JSON invalide, UTF-8 invalide) : la connexion est fermée,
  faute de pouvoir répondre à une requête précise. Avec un `id` : réponse d'erreur, la session continue.
- `Sanitize` : les 9 contrôles du §7 sont implémentés dans l'ordre. Précisions :
  - contrôle 2 : l'identité de la requête doit égaler celle du verdict, **et** le disque est relu par identité ;
  - contrôle 4 : comparaison **exacte** (casse comprise) des 4 derniers **caractères** du S/N ;
  - contrôle 5 : `detail` liste les codes manquants (`"D4,R7"`) pour que l'app les signale ;
  - contrôle 5 porte sur la méthode **choisie** (R7 exigé pour une écriture hôte en alternative) ;
  - au-delà des contrôles, l'exécution n'est pas encore implémentée (`INTERNAL`).
- `Unlock`, `CreateActivationRequest`, `InstallLicense`, `Cancel`, `GetJobResult` : pas encore implémentés ;
  `binary_sha256` est vide et la licence est `Missing` (donc tout `Sanitize` s'arrête au contrôle 8).
- Non encore fait : vérification `PKEXEC_UID` et purge de l'environnement (§3), événements, persistance (§10).
