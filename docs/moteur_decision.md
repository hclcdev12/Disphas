# Oblivio — Moteur de décision V1 (NVMe & SATA)

> Statut : **brouillon v0.1 — à valider**. Implémentation cible : crate `oblivio-core` (aucun accès disque).
> Les points marqués ⚠️ sont à confirmer contre le texte d'IEEE 2883-2022 avant figement.

## 0. Principe

Le moteur reçoit un **profil matériel** (lu par `oblivio-device`) et rend **un verdict unique** :
un état (§1), une méthode (§4), des réserves éventuelles, et la liste des raisons.
Il est déterministe : même profil + même version de règles = même verdict.
La version des règles (`rules_version`) est inscrite dans chaque certificat.

```
Profil ──► [1. Protections] ──► [2. Blocages] ──► [3. Santé / Say NO] ──► [4. Choix méthode] ──► Verdict
              PROTÉGÉ             BLOQUÉ            SAY NO / réserves        PURGE / CLEAR
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

Seule exception : si la résolution de B1 à B6 échoue de façon répétée **et** qu'aucune méthode Clear n'est possible, le disque bascule en SAY NO (raison `N9`).

## 3. Santé → SAY NO (rouge) ou réserves

### 3.1 Critères de refus (SAY NO)

| Code | Condition | Pourquoi |
|---|---|---|
| N1 | Inspection impossible : IDENTIFY en échec, erreur I/O sur commandes d'admin | Non vérifiable |
| N2 | NVMe Critical Warning bit 2 (fiabilité dégradée) | Le firmware déclare le média non fiable |
| N3 | NVMe Critical Warning bit 3 (média en lecture seule) | Aucune écriture ni validation par canaris possible |
| N4 | NVMe Critical Warning bit 4 (sauvegarde mémoire volatile en échec) | Contrôleur défaillant |
| N5 | ATA SMART RETURN STATUS = seuil dépassé (FAILING) | Le firmware prédit la panne |
| N6 | ATA attribut 5 (réalloués) ≥ **50** | Défaillance mécanique ou média avancée |
| N7 | Le nombre de secteurs réalloués **augmente pendant l'opération** | Dégradation active |
| N8 | Erreurs de lecture pendant la **validation** | Résultat non prouvable |
| N9 | Aucune méthode exécutable (ni Purge ni Clear) | — |
| N10 | Vérification ou validation en échec après **1** nouvelle tentative | Résultat non prouvable |

N7, N8 et N10 sont des **Say NO post-opération** : le certificat devient un rapport de refus.

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

## 4. Choix de la méthode

Le type de média est détecté via IDENTIFY : ATA mot 217 (= 1 → non rotatif), NVMe → flash.

### 4.1 NVMe (SSD)

Ordre de préférence :

| # | Méthode | Catégorie | Condition |
|---|---|---|---|
| 1 | Sanitize **Block Erase** | Purge | `SANICAP.BES` |
| 2 | Sanitize **Crypto Erase** | Purge | `SANICAP.CES` |
| 3 | Format NVM, SES = 2 (crypto) | Purge ⚠️ | `FNA` bit 2 |
| 4 | Format NVM, SES = 1 (user data) | Clear ⚠️ | Format supporté |
| 5 | Écriture hôte (zéros, 1 passe) | Clear + R7 | Toujours, si écriture possible |

- Block Erase passe avant Crypto Erase parce qu'il ne repose pas sur la confiance dans l'implémentation du chiffrement, invérifiable de l'extérieur. Option avancée : **CE puis BE** enchaînés.
- Sanitize **Overwrite** est exclu sur flash (usure, pas de gain).
- Sanitize touche **tous les namespaces** : mention explicite dans la confirmation et le certificat.

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
| 3 | Écriture hôte (zéros, 1 passe) | Clear + R7 | — |

### 4.4 Zones masquées

HPA (ou *Accessible Max Address* depuis ACS-4) détectée → **restaurée automatiquement** avant exécution, après accord explicite de l'opérateur, et inscrite au certificat.
DCO (obsolète depuis ACS-3) → signalée, pas modifiée en V1 (risque de rendre le disque inutilisable) ; catégorie plafonnée à Clear si une méthode hôte est utilisée.

## 5. Validation par méthode

| Méthode | Validation |
|---|---|
| Écriture hôte | Lecture des échantillons, contrôle du motif attendu (0x00) |
| Firmware (Sanitize, Format, Security Erase) | **Canaris** : avant l'opération, écriture d'un motif aléatoire unique (nonce) dans les blocs échantillons ; après, vérification qu'**aucun** canari ne subsiste |

Les canaris règlent le cas du Crypto Erase (lecture pseudo-aléatoire après coup) et celui des zones déjà vides.

**Échantillon** : LBA 0, LBA milieu, dernier LBA + **N** blocs aléatoires répartis (N = 1 000 par défaut, minimum 100). Graine aléatoire inscrite au certificat (rejouable).
Option « validation intégrale » (lecture complète) proposée pour les méthodes hôte.

## 6. Esquisse de types (`oblivio-core`)

```rust
pub enum Verdict {
    Protected { reasons: Vec<ProtectCode> },
    Blocked   { reasons: Vec<BlockCode>, guidance: Guidance },
    SayNo     { reasons: Vec<RefusalCode> },
    Ready     { method: Method, category: Category, reserves: Vec<ReserveCode> },
}

pub enum Category { Clear, Purge }   // Destruct n'est jamais exécuté par Oblivio

pub struct RuleSet {
    pub version: semver::Version,
    pub max_reallocated: u64,        // N6 (défaut 50)
    pub validation_samples: u32,     // défaut 1000, min 100
    pub max_retries: u8,             // N10 (défaut 1)
    // …
}

pub fn decide(profile: &DeviceProfile, rules: &RuleSet) -> Verdict;
```

## 7. Points ⚠️ à confirmer contre IEEE 2883-2022

- Catégorie de NVMe Format SES = 1 et SES = 2.
- Catégorie d'ATA Security Erase sur HDD et sur SSD.
- Exigences d'échantillonnage minimal pour la validation.
