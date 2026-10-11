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
Imprimé à **5,5 cm** avec sa marge blanche (4,4 cm jusqu'au 2026-10-11). Critère : un module d'au moins **0,40 mm**
imprimé, pour le certificat le plus long (test `largest_certificate_keeps_a_readable_qr_code` d'`oblivio-report`).
Un certificat typique fait environ 1 000 caractères (version 21), le plus long environ 1 300 (version 24) ; la
licence v2 ajoutera environ 240 caractères ([licences_en_ligne.md](licences_en_ligne.md) §3).

## 2. Contenu de l'attestation (signée)

| Clé | Contenu | Exemple |
|---|---|---|
| `v` | Version du schéma | `1` |
| `id` | Identifiant du rapport (UUID v7, triable par date) | `01928c3e-…` |
| `iat` | Fin de l'opération, UTC (epoch secondes) | — |
| `st` | **Statut global** (§3) | `CONFORME` |
| `dev` | Modèle, **numéro de série complet**, firmware, capacité (octets), type (NVMe / SATA-SSD / SATA-HDD), identifiants mondiaux si disponibles : WWN (ATA), EUI-64 / NGUID (NVMe) | — |
| `san` | Catégorie (`Clear` / `Purge` / `None`), méthode, portée « tous les namespaces » le cas échéant, référentiels (`NIST SP 800-88r2`, `IEEE 2883-2022`) | `Purge`, `NVMe Sanitize Block Erase` |
| `ver` | Vérification firmware : succès / échec | — |
| `val` | Validation : méthode (`canaris` / `motif`), nombre d'échantillons, graine, résultat | `canaris`, `1000`, `OK` |
| `dis` | **Devenir** : `Reuse` ou `DestroyRecommended` + codes (D1…D5) | — |
| `res` | Codes de réserves (R1…R8) | `["R4"]` |
| `rsn` | Codes de refus (N1…N10) pour REFUS et NON CONFORME *(ajouté le 2026-10-04 : le statut REFUS doit porter ses raisons)* | `["N1"]` |
| `frc` | Si forcé : justification saisie par l'opérateur | — |
| `org` / `op` | Organisation, opérateur | — |
| `lot` | Lot d'effacement (identifiant, libellé) | `LOT-2026-001`, `Commande 4512` |
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
| `dev` | `mdl`, `sn`, `fw`, `cap`, `typ` (`NVMe` / `SATA-SSD` / `SATA-HDD` / `SATA` / `Unknown`), `wwn`?, `eui`?, `ngu`? | `Unknown` : inspection impossible (N1 forcé) ; `SATA` (2026-10-06, banc de test) : disque SATA qui ne déclare pas son type de média (mot 217), effacé comme un SSD par prudence mais jamais certifié SSD ni HDD. **Liste ouverte** (décidé le 2026-10-07) : un type inconnu du vérificateur (1 à 64 caractères ASCII : lettres, chiffres, `-`, `_`) est accepté et affiché tel quel, comme un code ; un vérificateur ancien lit ainsi un type nouveau. Pour un tel type, `nsa` n'est pas tenu pour contradictoire |
| `san` | `cat` (`Clear` / `Purge` / `None`), `m`? (nom de méthode), `z`? (`true` : réécriture hôte terminée par une passe de zéros validée — absent sinon, `false` explicite rejeté), `nsa`? (`true` : la méthode a effacé **tous les namespaces** du contrôleur NVMe, pas seulement celui certifié — absent sinon, `false` explicite rejeté), `ref` (liste de référentiels) | `z` et `nsa` ajoutés le 2026-10-04 (avant toute mise sur le marché, d'où pas de nouvelle version de schéma) |
| `ver` | — | booléen, absent si rien n'a été exécuté |
| `val` | `m` (`canaris` / `motif`), `n`, `seed` (32 octets), `ok` | absent si rien n'a été exécuté |
| `dis` | `k` (`Reuse` / `DestroyRecommended`), `c` (codes D) | absent sur un refus |
| `lot`? | `id` (1 à 64 caractères ASCII : lettres, chiffres, `-`, `_`), `lbl`? (≤ 128 caractères) | lot d'effacement choisi par l'opérateur (déclaratif) ; prouve l'appartenance du disque au lot. Ajouté le 2026-10-04 (avant toute mise sur le marché, d'où pas de nouvelle version de schéma) ; hors bornes → attestation rejetée |
| `tool` | `ver`, `rules`, `bin` (SHA-256, 32 octets) | |
| `jh` | — | SHA-256 du journal, 32 octets |
| `lic` | — | la licence entière (COSE_Sign1 signé par la racine) |

- `?` = champ **omis** quand il n'a pas de valeur (jamais `null`) ; `res` et `rsn` toujours présents, éventuellement vides.
- **Décodage strict** : l'attestation décodée doit se réencoder à l'octet près. Champ inconnu, `null`, clés
  non triées, entier non minimal ou octets en trop → rejet.
- Codes et noms de méthode : 1 à 64 caractères `[A-Za-z0-9_]`. Les codes ne sont pas figés dans le format
  public, pour qu'un vérificateur ancien lise un code nouveau (il l'affichera sans libellé). Même règle pour le
  type de disque `dev.typ` (2026-10-07), avec `-` en plus.
- COSE : en-tête protégé `{alg: EdDSA}` **seul**, en-tête non protégé vide, message étiqueté (tag 18).
  Toute autre forme est rejetée (pas de confusion d'algorithme). Ed25519 en vérification stricte.
  L'enveloppe doit se **réencoder à l'octet près** (2026-10-04, revue de sécurité) : une même attestation signée
  n'a qu'un seul fichier possible, donc une seule empreinte SHA-256 ; une variante d'encodage est rejetée.
- QR : décompression bornée à 64 Kio (bombe zlib). Une attestation type tient sous 1 000 caractères (test).
- **Cohérence** (2026-10-04, revue de sécurité S7) : une attestation dont les champs se contredisent est
  rejetée au décodage, donc par tout vérificateur, même correctement signée ; le helper fait le même contrôle
  avant de signer et préfère alors l'échec de la tâche à un certificat contradictoire. Règles :
  une réécriture hôte (`m` = `HostOverwrite`, ou validation `motif`) n'est jamais `Purge` ;
  `z` seulement après une réécriture hôte vérifiée et validée ;
  `nsa` seulement sur un disque NVMe, ou d'un type inconnu du vérificateur, avec une méthode firmware (jamais une
  réécriture hôte) ;
  `CONFORME` : méthode, `ver` vrai, `val.ok` vrai, catégorie ≠ `None`, `dis` présent, `rsn` vide, pas de `frc` ;
  `NON_CONFORME` : `frc` et `rsn` non vide, catégorie ≠ `Purge` ;
  `REFUS` : catégorie `None`, `rsn` non vide, ni `dis` ni `z`.
  Textes déclaratifs : `op` ≤ 64, `org` ≤ 128, `frc` ≤ 500, `lot.lbl` ≤ 128 caractères, sans caractère de contrôle
  (sauf retour à la ligne dans `frc`) ni caractère de mise en forme invisible ou d'inversion de sens : un nom
  imprimé ne peut pas se lire autrement que ce qui est signé.

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
   │  stockée dans un fichier root 0600 (accès root uniquement), jamais exportée
   ▼
Attestation signée par la clé du poste, licence incluse
```

> **Licence v2** (décidée le 2026-10-11 ; format, vérification et installation implémentés le 2026-10-11, émission à
> venir ; détail : [licences_en_ligne.md](licences_en_ligne.md) §3) : signée par une **clé boutique** dont le
> *certificat d'émetteur* (signé par la racine : `iid` préfixé `ISS-`, `ipk`, `nbf`, `exp`, `off`, `maxd`) est
> embarqué dans la licence (`iss`, `v: 2`) ; `exp` obligatoire ; un même `id` par siège, conservé aux renouvellements.
> La liste de révocation reste en `v: 1` : une entrée `ISS-…` révoque un émetteur. Les licences v1 décrites
> ci-dessous restent acceptées. QR imprimé à 5,5 cm (§1).

**Licence** (CBOR déterministe, COSE_Sign1 par la racine) : `v`, `id`, `cli` (client), `off` (offre),
`nbf` (début de validité), `exp`? (fin, absente si perpétuelle), `wpk` (clé publique Ed25519 du poste, 32 octets).
La vérification exige `nbf ≤ iat ≤ exp`, et l'absence de révocation datée **au plus tard** à `iat`.

**Stockage de la clé du poste** (`oblivio-license`, décidé le 2026-10-04) : fichier `keys/workstation.key`
(en-tête `OBK1` + 32 octets), mode 0600, dans un dossier 0700 sous `/var/lib/oblivio`. Généré par le
générateur aléatoire de l'OS, **jamais écrasé**, refusé au chargement si le fichier ou son dossier est
ouvert à d'autres, ou si son propriétaire n'est pas celui du dossier. Aucune fonction ne restitue les octets secrets ;
effacé de la mémoire après usage. Pas de chiffrement en V1 : sans TPM ni mot de passe, une clé de
chiffrement stockée sur le même disque n'ajouterait rien aux permissions.
⚠️ Limite assumée : le vol du disque ou un accès root donnent la clé. Scellement TPM 2.0 à étudier (V2+).

**Formats texte** (copier-coller, courriel ou QR) :
- demande d'activation `OBA1:` + Base45 d'un COSE_Sign1 **signé par la clé du poste elle-même**
  (preuve de possession) : `{v, wpk, iat}` ;
- licence `OBL1:` + Base45 du COSE_Sign1 signé par la racine ; c'est le contenu du fichier `.lic`.
L'installation exige la signature de la racine, `wpk` = clé du poste, et une licence non expirée.
L'état est **revérifié à chaque lecture** : un `.lic` altéré, ou une clé de poste perdue, rend la licence invalide.

**Clé racine et émission** (`tools/oblivio-issuer`, outil de l'éditeur, jamais livré, 2026-10-04) :
- `oblivio-issuer keygen root.key` : paire Ed25519 tirée du générateur de l'OS ; graine chiffrée par
  ChaCha20-Poly1305 sous une clé dérivée de la phrase secrète par **Argon2id** (256 Mio, 3 passes) ; l'en-tête
  (paramètres, sel, nonce, clé publique) est authentifié. Fichier 0600, jamais écrasé, phrase ≥ 12 caractères,
  saisie au terminal sans écho.
- `oblivio-issuer issue root.key --customer … --offer … (--days N | --perpetual) 'OBA1:…'` : vérifie la preuve
  de possession de la demande, signe la licence `OBL1:…` et l'inscrit au registre `issued-licenses.csv`
  (base de la future liste de révocation). Identifiant `OBL-AAAAMMJJ-` + 64 bits aléatoires (16 chiffres
  hexadécimaux) ; un identifiant déjà au registre est refusé, généré ou donné par `--id` (la révocation porte sur
  l'identifiant : deux licences ne doivent jamais le partager).
- La clé publique officielle n'existe qu'en un seul endroit : `oblivio_attestation::trust::OFFICIAL_ROOT_KEY`,
  lue par le helper, l'application et `oblivio-verify`.
  **Générée le 2026-10-04** : `90241925111469b68676f8041578355a9f3e0bf14d84afd825f24782c3508a0b`.
- ⚠️ Recommandation : générer et conserver `root.key` sur une machine hors ligne ; deux sauvegardes sur supports
  distincts ; la phrase secrète conservée séparément. Perdre l'une ou l'autre interdit toute nouvelle licence.

**Activation offline** : le poste génère sa paire de clés → produit une demande d'activation
(fichier ou QR contenant la clé publique) → l'éditeur renvoie le `.lic` signé. Ce parcours reste le secours
hors ligne de l'activation en ligne décidée le 2026-10-11 ([licences_en_ligne.md](licences_en_ligne.md) §4).

**Révocation** : liste de licences révoquées signée par la racine, livrée avec les mises à jour.
Une attestation signée **avant** la date de révocation reste valide (horodatage `iat`).

Implémentation (2026-10-04) :
- **Format** (`oblivio_attestation::revocation`) : `OBR1:` + Base45 d'un COSE_Sign1 signé par la racine, sur le
  CBOR déterministe `{v: 1, seq, iat, rev: [[id, date], …]}`, identifiants triés et uniques, 100 000 entrées au plus.
  Une révocation est **définitive** : les listes ne font que grandir, et l'union de listes valides est toujours juste
  (pour une même licence, la date la plus ancienne l'emporte).
- **Émission** : `oblivio-issuer revoke root.key --id ID [--at EPOCH] (--list DERNIÈRE.obr | --first)
  [--register FICHIER] --out NOUVELLE.obr`. L'identifiant doit figurer au registre ; la nouvelle liste reprend la
  dernière (vérifiée) avec `seq + 1` ; fichier jamais écrasé. `--list` est obligatoire (une liste construite sans la
  précédente perdrait ses révocations) ; seule la toute première liste se fait avec `--first`, refusé dès qu'une
  liste est livrée, de même qu'une liste plus ancienne que celle livrée. `--at` permet d'antidater à la date de compromission d'une clé de poste : tout
  certificat signé depuis devient non authentique. Une date future est refusée.
- **Livraison** : la liste est copiée dans `public/oblivio-attestation/revocations.obr` et référencée par
  `trust::OFFICIAL_REVOCATIONS` ; un test vérifie à la compilation qu'elle est signée par la racine officielle.
- **Application** : le helper refuse de signer avec une licence révoquée (état `Revoked`) ; l'application l'applique à
  l'historique (état « licence révoquée »), à la vérification et au rendu PDF ; `oblivio-verify` applique la liste
  intégrée et, avec `--revocations FICHIER`, une liste plus récente signée par la **même** racine, et affiche le numéro
  de la liste la plus récente utilisée. Si la liste intégrée ne se vérifie pas, tout échoue (**fail closed**) : le helper
  ne signe plus et rien n'est déclaré authentique.

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
- Intégré à l'application (onglet « Vérifier », implémenté le 2026-10-04) : PDF (lecture de
  `attestation.cose` et `journal.json` joints, tailles bornées), `.cose` ou texte du QR collé. Mêmes contrôles
  que `oblivio-verify` ; un certificat non authentique est affiché par son motif seulement, jamais par son
  contenu. Un certificat signé par la racine de **développement** est signalé comme sans valeur en production ;
  un journal joint qui ne correspond pas à `jh` est signalé. Niveau *Audit* (relecture des échantillons) : à faire.
- `oblivio-verify` : vérificateur CLI **gratuit et open source**, dans un dépôt public séparé qui n'expose
  que le format de l'attestation et la vérification (voir [cadre](cadre_projet.md) §8).
- Le vérificateur affiche le résultat dans la langue de **son** utilisateur, quelle que soit la langue du PDF.

**`oblivio-verify` (implémenté le 2026-10-04)** :
`oblivio-verify [--lang en|fr|de] [--journal FICHIER] [--root-key HEX] (FICHIER | --qr TEXTE)`.
- Entrées : **PDF** du certificat, texte du QR (`OBV1:…`) ou attestation COSE binaire ; journal optionnel
  comparé à `jh`. D'un PDF, on vérifie l'attestation **jointe** (`attestation.cose`) et, sauf `--journal`, le
  journal joint ; un PDF sans attestation est **non authentique** (code 1). Lecture PDF (2026-10-04) dans
  `oblivio-attestation`, option `pdf` (dépendance `lopdf`, MIT ; Rust 1.88), partagée avec l'application.
- Codes de sortie : **0** authentique (et journal conforme s'il est fourni), **1** non authentique, **2** erreur d'usage.
- Langue : `--lang`, sinon celle du système, sinon anglais. Un code inconnu (Oblivio plus récent) est affiché brut.
- Clé racine officielle intégrée depuis le 2026-10-04 ; `--root-key` reste possible et le rapport avertit
  alors qu'il s'agit d'une clé non officielle (la liste intégrée, signée par la racine officielle, ne s'applique alors pas ;
  `--revocations` reste possible). La clé officielle donnée par `--root-key` reste officielle : liste intégrée
  appliquée, pas d'avertissement.

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

**Bibliothèque PDF : `typst`** (décidé le 2026-10-04, après spike). Les deux candidats produisent un
PDF/A-3b vérifié à l'export, avec fichiers joints et polices embarquées (typst écrit d'ailleurs ses PDF
avec krilla). typst l'emporte par son moteur de mise en page (retour à la ligne, tableaux, pagination,
gabarit `.typ` lisible, logo personnalisable) ; krilla obligeait à tout positionner à la main.
Coût accepté : ~40 Mo de binaire contre ~4 Mo, côté application utilisateur (jamais dans le helper root).

**Implémentation (`oblivio-report`)** :
- Rendu **depuis l'attestation signée, après vérification** : une attestation qui ne se vérifie pas, ou un
  journal qui ne correspond pas à `jh`, ne produit aucun PDF.
- PDF/A-3b, **déterministe** (identifiant et date dérivés de l'attestation : mêmes entrées, mêmes octets).
- Fichiers joints : `attestation.cose` (relation *Source*) et `journal.json` (relation *Supplement*).
- Polices embarquées : Inter et JetBrains Mono (OFL), sous-ensembles latin et latin-ext (`assets/fonts/`).
- Le gabarit ne reçoit que des **données** déjà traduites : un texte libre (organisation, justification)
  n'est jamais interprété comme du code typst. Téléchargement de paquets typst désactivé (aucun réseau).
- Annexe : graine de validation, version des règles, empreintes SHA-256 (binaire, journal, attestation).
- ⚠️ Conformité PDF/A affirmée par typst mais **pas encore validée par veraPDF** (validateur de référence,
  non installé) ; poppler émet un avertissement bénin sur `/Suspects`. À valider avant la première version.
- Non encore rendu : profil S.M.A.R.T. de l'annexe (le schéma du journal reste à définir).

### 6.1 Certificat de lot (décidé le 2026-10-04, option B)

Pour les gros volumes : **un PDF de synthèse** par lot, **en plus** des certificats individuels (chaque disque
reste prouvé par sa propre attestation signée).

- **Appartenance prouvée** : chaque attestation porte le numéro de lot (`lot`, §2) **sous sa signature**.
- **Un seul client par lot** (2026-10-04, revue de sécurité) : le numéro de lot est choisi par l'opérateur, deux
  clients peuvent donc utiliser le même ; un disque signé sous la licence d'un **autre client** (`cli` de la licence)
  que le premier disque est refusé, à la production comme à la vérification. Plusieurs licences (postes) d'un même
  client restent possibles dans un lot.
- **Contenu** : en-tête (lot, libellé, période de la première à la dernière opération), synthèse (nombre de
  CONFORME / NON CONFORME / REFUS — bandeau vert seulement si tout est CONFORME), capacité totale, organisation(s),
  opérateur(s), licence(s), outil ; puis **une ligne par disque** (n° de série, modèle, capacité, méthode,
  catégorie, statut, identifiant du rapport ; sous le n° de série, le premier identifiant matériel présent :
  WWN sur SATA, EUI-64 ou NGUID sur NVMe), dans l'ordre des opérations.
- **Production** : automatique à la **clôture du lot** dans l'application (refusée tant qu'un effacement du lot
  est en cours, sans quoi son certificat manquerait ; un lot vide se clôt sans PDF), ou à la demande depuis
  l'historique.
- **Pièces jointes** : pour chaque disque, `<rapport>.cose` (attestation signée) et `<rapport>.json` (journal) —
  un seul fichier suffit pour tout vérifier hors ligne. Pas de QR (un lot ne tient pas dans un QR) : chaque
  disque garde le sien sur son certificat individuel.
- **Refus de rendu** si une attestation ne se vérifie pas, si un journal ne correspond pas, si un disque n'a pas
  de lot ou appartient à un autre lot, ou s'il est fourni deux fois ; 1 à 2 000 disques par lot.
- PDF/A-3b, rendu déterministe (`oblivio-report::batch`).
- **Vérification** (`oblivio_attestation::batch`, partagée par l'application et `oblivio-verify`) : un PDF sans
  `attestation.cose` mais avec des `<rapport>.cose` est traité comme un lot ; chaque disque est vérifié
  (signature, licence, révocation), son journal comparé, son nom de fichier rapproché de son identifiant, et
  tous doivent porter le **même** lot. Le lot n'est authentique que si **chaque** disque l'est ; le résultat est
  donné disque par disque (code de sortie 0 seulement si tout est accepté).
- ⚠️ Limite assumée (option B) : le lot prouve l'**appartenance**, pas la **complétude** — retirer un disque du
  PDF n'est pas détectable. Preuve de complétude = option C (clôture signée par le helper), qui exigerait
  d'amender la règle invariante n° 5 de `CLAUDE.md`.

## 7. Points ouverts

- ~~Spike `krilla` vs `typst`~~ → typst (§6).
- Validation veraPDF des certificats produits.
- Contre-signature horodatée (option Grands Comptes).
