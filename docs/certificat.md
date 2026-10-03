# Oblivio — Certificat d'effacement : format, signature, vérification

> Statut : **brouillon v0.1 — à valider**. Crates : `oblivio-attestation` (public), `oblivio-license`, `oblivio-report`, signature dans `oblivio-helper`.
> Applique le modèle de confiance du [cadre](cadre_projet.md) §3 et les verdicts du [moteur de décision](moteur_decision.md).

## 1. Principe : un document, trois couches

```
┌───────────────────────────── PDF/A-3 (rendu lisible) ─────────────────────────────┐
│                                                                                   │
│   ┌──────────────────────────────┐        ┌──────────────────────────────────┐    │
│   │ ATTESTATION (signée)         │        │ JOURNAL DÉTAILLÉ (pièce jointe)  │    │
│   │ CBOR + COSE_Sign1 (Ed25519)  │──hash─►│ JSON : profil complet, logs,     │    │
│   │ → encodée dans le QR code    │        │ échantillons, chronologie        │    │
│   └──────────────────────────────┘        └──────────────────────────────────┘    │
│                                                                                   │
└───────────────────────────────────────────────────────────────────────────────────┘
```

| Couche | Format | Rôle |
|---|---|---|
| **Attestation** | CBOR déterministe (RFC 8949 §4.2), signée en **COSE_Sign1** (RFC 9052), algorithme EdDSA | Le cœur juridique. Champs essentiels + hash du journal. Assez compacte pour tenir dans un QR. |
| **Journal** | JSON, joint au PDF (PDF/A-3, fichier embarqué) | Le détail technique complet, pour l'auditeur. Son SHA-256 est dans l'attestation. |
| **PDF** | PDF/A-3 (archivage long terme) | Le rendu humain. **N'est jamais signé en tant que fichier** : il peut être réimprimé ou régénéré sans invalider la preuve. |

Le choix CBOR + COSE + QR suit le modèle éprouvé du certificat COVID numérique européen :
standards ouverts, vérification hors ligne, tient sur papier.

### Encodage du QR

`attestation COSE` → compression zlib → **Base45** (RFC 9285) → QR en mode alphanumérique, préfixe `OBV1:`.
Taille visée : < 1 000 caractères (QR version ~20, lisible imprimé en 4 × 4 cm).

## 2. Contenu de l'attestation (signée)

| Clé | Contenu | Exemple |
|---|---|---|
| `v` | Version du schéma | `1` |
| `id` | Identifiant du rapport (UUID v7, triable par date) | `01928c3e-…` |
| `iat` | Fin de l'opération, UTC (epoch secondes) | — |
| `st` | **Statut global** (§3) | `CONFORME` |
| `dev` | Modèle, **numéro de série complet**, firmware, capacité (octets), type (NVMe / SATA-SSD / SATA-HDD), identifiants mondiaux si disponibles : WWN (ATA), EUI-64 / NGUID (NVMe) | — |
| `san` | Catégorie (`Clear` / `Purge` / `None`), méthode, référentiels (`NIST SP 800-88r2`, `IEEE 2883-2022`) | `Purge`, `NVMe Sanitize Block Erase` |
| `ver` | Vérification firmware : succès / échec | — |
| `val` | Validation : méthode (`canaris` / `motif`), nombre d'échantillons, graine, résultat | `canaris`, `1000`, `OK` |
| `dis` | **Devenir** : `Reuse` ou `DestroyRecommended` + codes (D1…D5) | — |
| `res` | Codes de réserves (R1…R8) | `["R4"]` |
| `rsn` | Codes de refus (N1…N10) pour REFUS et NON CONFORME *(ajouté le 2026-10-04 : le statut REFUS doit porter ses raisons)* | `["N1"]` |
| `frc` | Si forcé : justification saisie par l'opérateur | — |
| `org` / `op` | Organisation, opérateur | — |
| `tool` | Version d'Oblivio, `rules_version`, SHA-256 du binaire | — |
| `jh` | SHA-256 du journal détaillé | — |
| `lic` | Licence du poste (elle-même signée par la racine Oblivio, §4) | — |

Le numéro de série figure **en entier** dans l'attestation signée et dans le PDF : c'est lui qui lie la preuve
à un disque physique. Les identifiants mondiaux (WWN, EUI-64, NGUID), attribués par le constructeur et
uniques au monde, renforcent ce lien quand le disque les expose.

L'attestation ne contient **aucun texte traduit**, uniquement des codes : la langue relève du rendu (§6).

### Encodage détaillé (`oblivio-attestation`, décidé le 2026-10-04)

| Clé | Sous-clés | Notes |
|---|---|---|
| `id` | — | UUID v7 en 16 octets bruts (pas en texte : économise ~20 octets dans le QR) |
| `dev` | `mdl`, `sn`, `fw`, `cap`, `typ` (`NVMe` / `SATA-SSD` / `SATA-HDD`), `wwn`?, `eui`?, `ngu`? | |
| `san` | `cat` (`Clear` / `Purge` / `None`), `m`? (nom de méthode), `ref` (liste de référentiels) | |
| `ver` | — | booléen, absent si rien n'a été exécuté |
| `val` | `m` (`canaris` / `motif`), `n`, `seed` (32 octets), `ok` | absent si rien n'a été exécuté |
| `dis` | `k` (`Reuse` / `DestroyRecommended`), `c` (codes D) | absent sur un refus |
| `tool` | `ver`, `rules`, `bin` (SHA-256, 32 octets) | |
| `jh` | — | SHA-256 du journal, 32 octets |
| `lic` | — | la licence entière (COSE_Sign1 signé par la racine) |

- `?` = champ **omis** quand il n'a pas de valeur (jamais `null`) ; `res` et `rsn` toujours présents, éventuellement vides.
- **Décodage strict** : l'attestation décodée doit se réencoder à l'octet près. Champ inconnu, `null`, clés
  non triées, entier non minimal ou octets en trop → rejet.
- Codes et noms de méthode : 1 à 64 caractères `[A-Za-z0-9_]`. Les codes ne sont pas figés dans le format
  public, pour qu'un vérificateur ancien lise un code nouveau (il l'affichera sans libellé).
- COSE : en-tête protégé `{alg: EdDSA}` **seul**, en-tête non protégé vide, message étiqueté (tag 18).
  Toute autre forme est rejetée (pas de confusion d'algorithme). Ed25519 en vérification stricte.
- QR : décompression bornée à 64 Kio (bombe zlib). Une attestation type tient sous 1 000 caractères (test).

## 3. Les trois statuts

| Statut | Quand | Rendu PDF |
|---|---|---|
| **CONFORME** | Verdict PRÊT, opération exécutée, vérification **et** validation OK | Bandeau vert. Si `dis = DestroyRecommended` : second bandeau rouge « Devenir recommandé : destruction physique » |
| **NON CONFORME** | Opération **forcée** sur un disque SAY NO | Bandeau rouge. Catégorie plafonnée à Clear. Justification affichée en tête |
| **REFUS** | SAY NO sans forçage, ou SAY NO après opération (N8, N10) | Bandeau rouge « Effacement non prouvable — destruction physique requise ». Raisons détaillées |

Un disque PROTÉGÉ ou BLOQUÉ ne produit **aucun** document : il n'y a eu ni tentative ni refus.

## 4. Chaîne de signature

```
Racine Oblivio (Ed25519, hors ligne, éditeur)
   │  signe (COSE_Sign1)
   ▼
Licence .lic = { id, client, offre, validité, clé publique du poste }
   │  la clé privée correspondante est générée sur le poste à l'activation,
   │  stockée chiffrée (accès root uniquement), jamais exportée
   ▼
Attestation signée par la clé du poste, licence incluse
```

**Licence** (CBOR déterministe, COSE_Sign1 par la racine) : `v`, `id`, `cli` (client), `off` (offre),
`nbf` (début de validité), `exp`? (fin, absente si perpétuelle), `wpk` (clé publique Ed25519 du poste, 32 octets).
La vérification exige `nbf ≤ iat ≤ exp`, et l'absence de révocation datée **au plus tard** à `iat`.

**Activation offline** : le poste génère sa paire de clés → produit une demande d'activation
(fichier ou QR contenant la clé publique) → l'éditeur renvoie le `.lic` signé.

**Révocation** : liste de licences révoquées signée par la racine, livrée avec les mises à jour.
Une attestation signée **avant** la date de révocation reste valide (horodatage `iat`).

> ⚠️ Limite assumée : `iat` est déclaré par le poste, sans horodatage tiers (offline-first).
> Option Grands Comptes à étudier : contre-signature horodatée RFC 3161 lors d'une connexion ultérieure.

## 5. Vérification

Trois niveaux, tous hors ligne, avec la seule clé publique racine :

| Niveau | Entrée | Ce qui est prouvé |
|---|---|---|
| **Papier** | Scan du QR | L'attestation est authentique et intacte ; les champs affichés par le vérificateur doivent correspondre au papier |
| **PDF** | Fichier PDF | Idem + le journal joint correspond au hash `jh` |
| **Audit** | PDF + journal | Idem + relecture complète des échantillons et de la chronologie |

**Outils de vérification :**
- Intégré à l'application (menu « Vérifier un certificat »).
- `oblivio-verify` : vérificateur CLI **gratuit et open source**, dans un dépôt public séparé qui n'expose
  que le format de l'attestation et la vérification (voir [cadre](cadre_projet.md) §8).
- Le vérificateur affiche le résultat dans la langue de **son** utilisateur, quelle que soit la langue du PDF.

## 6. Mise en page du PDF

**Langue** : EN, FR ou DE, choisie à la génération (défaut : langue de l'application).
Un même certificat peut être régénéré dans une autre langue à partir de l'attestation et du journal,
sans nouvelle signature. Les termes normatifs (*Clear*, *Purge*…) restent en anglais.


**Page 1 — Synthèse** (fond blanc, structures `#111827`, accents de la charte) :
1. En-tête : logo (personnalisable en offre Atelier), titre « Certificat d'effacement », identifiant, date.
2. Bandeau(x) de statut (§3).
3. Bloc support : modèle, S/N, firmware, capacité, type.
4. Bloc sanitisation : catégorie, méthode, référentiels, vérification, validation.
5. Bloc devenir et réserves, en langage clair.
6. Pied : organisation, opérateur, outil, licence, **QR code**, empreinte courte de l'attestation.

**Pages suivantes — Annexe technique** : profil S.M.A.R.T., chronologie horodatée, paramètres d'échantillonnage, mentions légales.

**Bibliothèque PDF** : à trancher par un spike court entre `krilla` (bas niveau, PDF/A et fichiers joints) et `typst` en bibliothèque (gabarits élégants, personnalisation du logo facilitée). Critères : conformité PDF/A-3, fichiers joints, polices embarquées, taille du binaire.

## 7. Points ouverts

- Spike `krilla` vs `typst`.
- Contre-signature horodatée (option Grands Comptes).
