# Oblivio — Moteur de décision V1 (NVMe & SATA)

> Statut : **brouillon v0.1 — à valider**. Implémentation cible : crate `oblivio-core` (aucun accès disque).
> Les points marqués ⚠️ sont à confirmer contre le texte d'IEEE 2883-2022 avant figement.

## 0. Principe

Le moteur reçoit un **profil matériel** (lu par `oblivio-device`) et rend **un verdict unique** :
un état (§1), une méthode (§4), des réserves éventuelles, un devenir (§3.3), et la liste des raisons.
Il est déterministe : même profil + même version de règles = même verdict.
La version des règles (`rules_version`) est inscrite dans chaque certificat.

```
Profil ──► [1. Protections] ──► [2. Blocages] ──► [3. Santé / Say NO] ──► [4. Choix méthode] ──► Verdict
              PROTÉGÉ             BLOQUÉ        SAY NO / devenir / réserves   PURGE / CLEAR
```

Chaque étape court-circuite : le premier état non-vert l'emporte.

## 1. Protections → PROTÉGÉ (gris)

Le disque est intouchable si l'un de ces cas est vrai (vérifié **dans le helper**) :

| Code | Condition | Source |
|---|---|---|
| P1 | Héberge `/`, `/boot`, `/boot/efi` ou le support de démarrage | `/proc/self/mountinfo`, sysfs |
| P2 | Une partition est montée | `/proc/self/mountinfo` |
| P3 | Une partition est en swap actif | `/proc/swaps` |
| P4 | Membre d'un volume LVM / RAID md / dm-crypt / multipath | `/sys/block/*/holders` |
| P5 | NVMe : un **autre namespace** du même contrôleur est concerné par P1–P4 (Sanitize agit sur tout le contrôleur) | Identify + sysfs |

**Détection** (`oblivio-device::inventory`, décidé le 2026-10-04) : lecture seule de `/sys` et `/proc`,
aucun nœud `/dev` ouvert, aucun ioctl. Seule exception (S11, revue de sécurité) : un point de montage système
monté à la demande (autofs, `/boot/efi` par exemple) est listé pour déclencher son montage, comme le ferait
n'importe quel programme qui le lit ; s'il reste non monté, l'inventaire **échoue**.
- Les montages sont rattachés aux disques par `major:minor`, puis en remontant les `slaves/` des
  périphériques empilés (dm-crypt, LVM, md) : une racine sur LUKS + LVM désigne bien le disque physique.
  Pour btrfs (numéro de périphérique anonyme), la source `/dev/…` du montage est utilisée.
- P1 : points de montage `/`, `/boot`, `/boot/efi`, `/efi`.
- P4 (S9) : un périphérique loop adossé directement à un disque ou à une partition (`loop/backing_file` en
  `/dev/…`) compte comme P4 ; un système de fichiers monté par un tel loop est rattaché au disque dessous. Un loop
  sur un fichier ordinaire relève du montage qui porte ce fichier.
- P5 : namespaces d'un même contrôleur, d'après le nom (`nvme0n1`, `nvme0n2` → `nvme0`).
- NVMe (S10) : seul le transport `pcie` du contrôleur (`device/transport`, ou chacun des contrôleurs du
  sous-système en multipath) est un disque local ; NVMe sur réseau (`tcp`, `rdma`, `fc`) ou cible `loop` → transport
  « autre », jamais ouvert pour effacement.
- À l'ouverture pour effacement (S9) : refus si un autre processus a le disque, une de ses partitions ou un
  namespace voisin tenu ouvert (`/proc/<pid>/fd`) — `O_EXCL` n'arrête que les ouvertures exclusives.
- **Échec fermé** : `mountinfo` ou `swaps` illisible, swap ou montage `/dev/…` introuvable, ou racine `/`
  rattachée à aucun disque (ZFS, overlay qu'on ne sait pas suivre…) → l'inventaire **échoue** entièrement.
  Aussi (2026-10-04, revue de sécurité) : un montage dont le numéro de périphérique n'existe pas dans sysfs, une
  pile de périphériques illisible ou plus profonde que 16 niveaux, et **tout** montage ZFS (un pool ne se rattache
  pas à ses disques par la table des montages) : leurs disques auraient paru libres.
  Un dossier `holders/` illisible compte comme P4.
- Périmètre V1 : seuls `nvme<N>n<M>` et `sd<lettres>` sont listés ; le transport (NVMe, ATA, USB) est signalé.
  ⚠️ L'échec sur racine ZFS rend l'application inutilisable sur ces systèmes : à revoir avec l'ISO (V2).
- **Système live** (décidé le 2026-10-06, banc de test démarré en PXE) : une racine `overlay` est suivie couche par
  couche (`lowerdir`, `upperdir`, `lowerdir+`, `datadir+`) jusqu'au montage qui porte chaque couche, et un loop adossé
  à un **fichier** jusqu'au montage qui porte ce fichier. Les disques trouvés sont le disque système (P1) : une clé live
  USB, une partition de persistance. Si **toutes** les couches finissent en RAM ou sur une autre machine (`tmpfs`,
  `ramfs`, `nfs`, `nfs4`, `cifs`, `smb3`), il n'y a **pas de disque système local** et l'inventaire l'accepte ; une
  racine NFS directe aussi. Toute couche introuvable laisse la racine inconnue → échec (*fail closed*). Vérifié sur le
  banc (live PXE : squashfs sur NFS, couche d'écriture en RAM) et sur le poste de développement.

## 2. Blocages → BLOQUÉ (orange)

Disque sain mais action impossible en l'état. Oblivio guide l'opérateur, ne prescrit **jamais** la destruction.

| Code | Condition | Guidage proposé |
|---|---|---|
| B1 | ATA Security **Frozen** (IDENTIFY mot 128 bit 3) | Mise en veille/réveil (S3) puis réinspection |
| B2 | ATA Security **Locked** (mot 128 bit 2) | Saisie du mot de passe utilisateur/maître |
| B3 | ATA Sanitize **Freeze Lock** actif | Cycle d'alimentation |
| B4 | Sanitize précédent **en cours** | Attendre (progression lue dans le log), reprise automatique |
| B5 | Sanitize précédent **en échec** (mode Failure) | Commande d'Exit Failure Mode puis nouvelle tentative |
| B6 | Compteur de tentatives de mot de passe expiré (mot 128 bit 4) | Cycle d'alimentation |
| B7 | Sécurité ATA **activée par un tiers** (mot 128 bit 1) sans verrouillage | Retirer le mot de passe existant, puis réinspection. Oblivio ne remplace jamais un mot de passe qu'il n'a pas posé (décidé le 2026-10-04) |

**Portée d'un blocage** (décidé le 2026-10-03) :
- B2, B4, B5, B6 rendent **toutes** les méthodes indisponibles : ils bloquent toujours.
- B1 (*Frozen*) et B7 n'interdisent que Security Erase, B3 (*Freeze Lock*) que l'ATA Sanitize.
  Ils ne bloquent que s'ils empêchent la **méthode préférée** ; sinon le disque est PRÊT
  et la méthode concernée est simplement retirée des alternatives.

**Échecs répétés** : après `max_unblock_attempts` résolutions échouées (paramètre du `RuleSet`, défaut **3**),
le moteur se rabat sur la meilleure méthode encore exécutable (par exemple l'écriture hôte, Clear + R7).
S'il n'en reste aucune, le disque bascule en SAY NO (raison `N9`).

## 3. Santé → SAY NO (rouge) ou réserves

**Principe** (décidé le 2026-10-03, conforme au cadre §4) : SAY NO signifie « effacement **non prouvable** ».
Un disque en mauvaise santé dont l'effacement reste prouvable est PRÊT : l'effacement est certifié
et le certificat porte un devenir « Destruction recommandée » (§3.3).

### 3.1 Critères de refus (SAY NO)

| Code | Condition | Pourquoi |
|---|---|---|
| N1 | Inspection impossible : IDENTIFY en échec, erreur I/O sur commandes d'admin | Non vérifiable |
| N3 | NVMe Critical Warning bit 3 (média en lecture seule) | Aucune écriture ni validation par canaris possible |
| N8 | Erreurs de lecture pendant la **validation** | Résultat non prouvable |
| N9 | Aucune méthode exécutable (ni Purge ni Clear) | — |
| N10 | Vérification ou validation en échec après `max_retries` nouvelle(s) tentative(s) (défaut **1**) | Résultat non prouvable |

N8 et N10 sont des **Say NO post-opération** : le certificat devient un rapport de refus.

**Évaluation post-opération** (décidé le 2026-10-04), après chaque tentative :
1. **N8** si un échantillon est illisible, ou si les attributs ATA 197/198 sont > 0 après l'opération.
   Refus immédiat, **sans** nouvelle tentative : relancer ne rendrait pas le résultat prouvable.
2. Vérification firmware en échec ou échantillon non conforme → nouvelle tentative tant que
   `max_retries` n'est pas atteint, puis **N10**.
3. Sinon **CONFORME** ; **D5** s'ajoute au devenir si l'attribut ATA 5 a augmenté pendant l'opération.

Codes **retirés** (non réattribués) : N2 → D1, N4 → D2, N5 → D3, N6 → D4, N7 → D5.

### 3.2 Réserves (vert avec avertissement)

Le disque reste PRÊT mais la réserve est **affichée dans l'UI et inscrite au certificat**.

| Code | Condition |
|---|---|
| R1 | NVMe *Available Spare* < seuil constructeur (Critical Warning bit 0) |
| R2 | NVMe *Percentage Used* ≥ 100 % (endurance épuisée, pas un défaut) |
| R3 | NVMe *Media and Data Integrity Errors* > 0 |
| R4 | ATA attribut 5 entre 1 et 49 |
| R5 | ATA 197 (en attente) ou 198 (incorrigibles) > 0 — **si encore > 0 après opération → N8** |
| R6 | ATA 187 (erreurs non corrigées signalées) > 0 |
| R7 | Méthode Clear uniquement : les secteurs réalloués ne sont pas couverts par une écriture hôte |
| R8 | Modèle/firmware présent dans la base de quirks Oblivio |

Tous les seuils (50, nombre de tentatives…) sont des **paramètres versionnés** de `rules_version`, pas des constantes dans le code.

### 3.3 Devenir (PRÊT, destruction recommandée)

L'effacement est exécuté, vérifié, validé et certifié **CONFORME**. Le certificat porte en plus
`dis = DestroyRecommended` avec les codes ci-dessous (second bandeau rouge, voir [certificat.md](certificat.md) §3).
Sans code D, le devenir est `Reuse`. L'opérateur prend acte des codes D avant l'effacement (`acknowledged`).

| Code | Condition | Pourquoi |
|---|---|---|
| D1 | NVMe Critical Warning bit 2 (fiabilité dégradée) | Le firmware déclare le média non fiable |
| D2 | NVMe Critical Warning bit 4 (sauvegarde mémoire volatile en échec) | Contrôleur défaillant |
| D3 | ATA SMART RETURN STATUS = seuil dépassé (FAILING) | Le firmware prédit la panne |
| D4 | ATA attribut 5 (réalloués) ≥ `max_reallocated_sectors` (défaut **50**) | Défaillance mécanique ou média avancée |
| D5 | Le nombre de secteurs réalloués **augmente pendant l'opération** | Dégradation active (**post-opération**) |

Un SAY NO l'emporte toujours : un disque N3 et D1 est un refus, pas un PRÊT.

### 3.4 Forçage d'un SAY NO (décidé le 2026-10-04)

Applique le [cadre](cadre_projet.md) §5.3. Depuis l'introduction des codes D, les disques en fin de vie
mais effaçables sont PRÊT : le forçage ne concerne plus que l'inspection impossible.

| Refus | Forçable | Méthode | Raison |
|---|---|---|---|
| N1 | **Oui** | Écriture hôte, Clear + R7, validation par motif | Le disque peut encore accepter les écritures |
| N3 | Non | — | Aucune écriture, donc aucune validation possible |
| N9 | Non | — | Aucune méthode à exécuter |
| N8, N10 | Non | — | Post-opération : trop tard |

- Le verdict SAY NO indique s'il est forçable (`forcible`) : l'UI n'affiche le bouton que dans ce cas.
- Justification d'au moins `min_force_justification_chars` caractères (défaut **20**, espaces de bord exclus), conservée telle quelle.
- Une tentative forcée subit les mêmes contrôles post-opération (§3.1) :
  succès → **NON CONFORME** (jamais CONFORME) ; échec → **REFUS** avec les raisons d'origine plus N8 ou N10.

## 4. Choix de la méthode

Le type de média est détecté via IDENTIFY : ATA mot 217 (= 1 → non rotatif), NVMe → flash.

Les catégories marquées ⚠️ sont des **paramètres du `RuleSet`** (`categories`), avec pour valeurs par défaut
celles des tableaux ci-dessous : leur confirmation contre IEEE 2883 ne changera que `rules_version`, pas le code.
L'écriture hôte, elle, est **toujours** Clear, quel que soit le `RuleSet` (règle invariante).

**Décodage** (`oblivio-device::decode`, décidé le 2026-10-04) : fonctions pures, octets → profil.
- Tampon trop court, somme de contrôle IDENTIFY ATA (mot 255, signature A5h) ou SMART fausse → **rejeté**, jamais interprété.
- Bits Sanitize ATA (mot 59, bits 13 à 15) ignorés si le bit 12 (jeu Sanitize supporté) est absent.
- Mot 217 non renseigné : disque traité comme **SSD** (jamais d'Overwrite proposé, Security Erase certifié Clear
  seulement). L'indice `rotational` de sysfs n'est **pas** utilisé (corrigé le 2026-10-06, banc de test) : pour un
  disque SATA, le noyau le tire du même mot 217 et répond « rotatif » quand il manque, ce qui aurait fait certifier
  Purge le Security Erase d'un SSD muet sur son type.
- Attributs SMART lus en brut 48 bits, comme `smartctl` par défaut : un constructeur qui y range autre chose
  gonfle la valeur, ce qui va dans le sens prudent (D4). La base de quirks corrigera les modèles connus.
- ⚠️ Offsets et bits issus des normes (NVMe Base 2.0, ACS-3/4), pas encore confrontés à de vrais disques :
  tests différentiels contre `smartctl --json -x` à faire sur le banc (cadre §9). À vérifier en priorité :
  bits de SANITIZE STATUS EXT (14 = en cours, 13 = gelé), FNA et SANICAP.

**Exécution** (`oblivio-device`, 2026-10-04) : NVMe Sanitize (Block / Crypto Erase) et Format NVM (SES 1/2)
par commandes d'administration ; ATA Sanitize (Block Erase, Crypto Scramble, Overwrite 1 passe à zéro) par
ATA PASS-THROUGH (16) ; progression lue dans le journal / SANITIZE STATUS EXT.

**ATA Security Erase** (2026-10-04) : seulement sur un disque **sans mot de passe** (sinon B7, le disque
appartient à un tiers). Le helper tire un mot de passe utilisateur **aléatoire** (32 caractères
alphanumériques, jamais journalisé, `zeroize`), l'**enregistre durablement avant** de le poser
(`<state>/recovery/ata-<hex(S/N)>.json`, dossier 0700, fichier 0600, écriture atomique + fsync), pose le
mot de passe, vérifie que la sécurité est bien activée, lance SECURITY ERASE PREPARE + UNIT (Enhanced si
choisi ; délai = 2 × l'estimation des mots 89/90, 60 min minimum). Succès seulement si la sécurité est
**désactivée** après coup (la norme l'exige) ; sinon le helper déverrouille et retire son mot de passe.
L'enregistrement n'est supprimé qu'une fois le disque libéré : s'il en reste un au démarrage (coupure,
plantage), le helper restaure le disque avant toute autre chose et garde l'enregistrement en cas d'échec
(un technicien peut s'en servir avec des outils standards). Aucune progression : l'opération est bloquante.

### 4.1 NVMe (SSD)

Ordre de préférence :

| # | Méthode | Catégorie | Condition |
|---|---|---|---|
| 1 | Sanitize **Block Erase** | Purge | `SANICAP.BES` |
| 2 | Sanitize **Crypto Erase** | Purge | `SANICAP.CES` |
| 3 | Format NVM, SES = 2 (crypto) | Purge ⚠️ | `FNA` bit 2 |
| 4 | Format NVM, SES = 1 (user data) | Clear ⚠️ | Format supporté |
| 5 | Écriture hôte (flux pseudo-aléatoire, 1 passe) | Clear + R7 | Toujours, si écriture possible |

- Block Erase passe avant Crypto Erase parce qu'il ne repose pas sur la confiance dans l'implémentation du chiffrement, invérifiable de l'extérieur. Option avancée : **CE puis BE** enchaînés.
- Sanitize **Overwrite** est exclu sur flash (usure, pas de gain).
- Sanitize touche **tous les namespaces** : mention explicite dans la confirmation et le certificat.
  Format NVM aussi quand le contrôleur l'annonce (`FNA` bit 1) : le verdict porte alors la même portée
  (`all_namespaces`, prise d'acte `NS_ALL`). Pendant une opération à l'échelle du contrôleur, le helper tient
  les autres namespaces en `O_EXCL` et refuse si l'un d'eux est utilisé.
  Le certificat la porte sous la signature (`san.nsa`, décidé le 2026-10-04) : posé dès que la méthode a été
  exécutée, même si le résultat est un REFUS, et affiché « Portée : tous les namespaces du contrôleur NVMe ».

### 4.2 SATA SSD (non rotatif)

| # | Méthode | Catégorie | Condition |
|---|---|---|---|
| 1 | ATA Sanitize **Block Erase** | Purge | IDENTIFY mot 59 bit 15 |
| 2 | ATA Sanitize **Crypto Scramble** | Purge | Mot 59 bit 13 |
| 3 | ATA Security Erase Unit (Enhanced) | **Clear** par défaut ⚠️ | Mot 128 bit 5 — Purge seulement si validé par la base de quirks |
| 4 | Écriture hôte | Clear + R7 | — |

Les implémentations de Security Erase sur SSD ont historiquement été peu fiables : on ne les certifie pas Purge par défaut.

### 4.3 SATA HDD (rotatif)

| # | Méthode | Catégorie | Condition |
|---|---|---|---|
| 1 | ATA Sanitize **Overwrite** | Purge | Mot 59 bit 14 |
| 2 | ATA Security Erase Unit (Enhanced, sinon Normal) | Purge ⚠️ | Mot 128 |
| 3 | Écriture hôte (flux pseudo-aléatoire, 1 passe) | Clear + R7 | — |

### 4.4 Zones masquées

HPA (ou *Accessible Max Address* depuis ACS-4) détectée → **restaurée automatiquement** avant exécution, après accord explicite de l'opérateur, et inscrite au certificat.
DCO (obsolète depuis ACS-3) → signalée, pas modifiée en V1 (risque de rendre le disque inutilisable) ; catégorie plafonnée à Clear si une méthode hôte est utilisée.

## 5. Validation par méthode

| Méthode | Validation |
|---|---|
| Écriture hôte | Lecture des échantillons, contrôle du motif attendu : le flux pseudo-aléatoire recalculé à la position de chaque bloc (voir ci-dessous) |
| Firmware (Sanitize, Format, Security Erase) | **Canaris** : avant l'opération, écriture d'un motif aléatoire unique (nonce) dans les blocs échantillons ; après, vérification qu'**aucun** canari ne subsiste |

Les canaris règlent le cas du Crypto Erase (lecture pseudo-aléatoire après coup) et celui des zones déjà vides.

**Flux de la réécriture hôte** (décidé le 2026-10-04, alignement BSI IT-Grundschutz CON.6.A12, voir
[conformité](conformite.md)) : une passe complète d'un flux **ChaCha20** (RFC 8439) dont la clé est
SHA-256(graine ‖ « overwrite »), la graine étant celle de la validation, tirée du générateur de l'OS et
signée dans l'attestation. L'octet *n* du disque reçoit l'octet *n* du flux : la validation recalcule le
contenu attendu de chaque bloc échantillon et l'exige **à l'identique** (un bloc laissé à zéro ou intact
échoue). Remplace la passe de zéros. Débit mesuré du générateur : ~2 Go/s sur un cœur (Ryzen AI 9 365),
au-dessus d'un disque dur ou d'un SSD SATA ; sur un NVMe rapide il peut limiter la réécriture (méthode de
repli) — parallélisation possible plus tard.
**Option « finir par une passe de zéros »** (décidée et implémentée le 2026-10-04 ; `Sanitize.zero_pass`, refusée hors réécriture hôte ; champ signé `san.z`) : pour les clients
qui veulent un disque d'apparence vide. Passe pseudo-aléatoire, **validation**, puis passe de zéros et
seconde validation : les deux passes sont prouvées et le certificat l'indique. Désactivée par défaut
(elle double la durée : ~11 h au lieu de ~5 h 30 pour un disque dur de 4 To à 200 Mo/s, estimation à
mesurer sur le banc).

**Échantillon** : LBA 0, LBA milieu, dernier LBA + **N** blocs aléatoires répartis (N = 1 000 par défaut, minimum 100). Graine aléatoire inscrite au certificat (rejouable).
Option « validation intégrale » (lecture complète) proposée pour les méthodes hôte.

## 6. Esquisse de types (`oblivio-core`)

Implémenté dans `crates/oblivio-core` (rules_version **1.0.0**). Forme simplifiée :

```rust
pub enum Verdict {
    Protected { reasons: Vec<ProtectCode> },
    Blocked   { reasons: Vec<BlockCode> },        // guidage : BlockCode::guidance()
    SayNo     { reasons: Vec<RefusalCode> },
    Ready(Ready),
}

pub struct Ready {
    pub recommended: MethodOption,                // { method, category }
    pub alternatives: Vec<MethodOption>,          // méthodes autorisées, par préférence
    pub drive_reserves: Vec<ReserveCode>,         // réserves du disque ; reserves_for(méthode) ajoute R7
    pub disposition: Disposition,                 // Reuse | DestroyRecommended { reasons: Vec<DispositionCode> }
}

pub enum Category { Clear, Purge }   // Destruct n'est jamais exécuté par Oblivio

pub struct RuleSet {
    pub version: semver::Version,
    pub max_reallocated_sectors: u64, // D4 (défaut 50)
    pub max_unblock_attempts: u32,    // N9 (défaut 3)
    pub max_retries: u8,              // N10 (défaut 1)
    pub validation_samples: u32,      // défaut 1000
    pub min_validation_samples: u32,  // défaut 100
    pub categories: CategoryRules,    // catégories ⚠️ (§4)
}

pub fn decide(profile: &DeviceProfile, rules: &RuleSet) -> Verdict;
```

`decide()` couvre les étapes 1 à 4. R7 dépend de la méthode **réellement choisie** par l'opérateur,
pas de la méthode recommandée : `Ready::reserves_for(method)` donne la liste à acquitter et à certifier.

```rust
pub fn evaluate_attempt(profile, ready, executed: Method, retries_used: u8,
                        report: &AttemptReport, rules) -> Result<Evaluation, EvaluationError>;

pub enum Evaluation { Retry, Compliant(Compliant), Refused { reasons: Vec<RefusalCode> } }
```

`evaluate_attempt()` applique l'évaluation post-opération (§3.1) et produit N8, N10 et D5.
`force(verdict, justification, rules)` accepte ou rejette un forçage (§3.4) ; `evaluate_forced_attempt()`
évalue une tentative forcée et rend `NonCompliant` ou `Refused`, jamais `Compliant`.

## 7. Points ⚠️ à confirmer contre IEEE 2883-2022

- Catégorie de NVMe Format SES = 1 et SES = 2.
- Catégorie d'ATA Security Erase sur HDD et sur SSD.
- Exigences d'échantillonnage minimal pour la validation.
