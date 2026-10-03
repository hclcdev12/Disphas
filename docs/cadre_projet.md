# Oblivio — Cadre du projet (v0.1)

> Document de référence. Il prime sur les autres specs en cas de contradiction.
> Validé le 2026-10-03.

## 1. Périmètre V1

**Inclus**
- Application desktop **Linux 64-bit** (Tauri v2 + moteur Rust).
- Supports **NVMe** et **SATA** en connexion directe (carte mère).
- Parcours complet : inspection → décision → exécution → vérification → validation → certificat PDF signé.
- Licences offline (`.lic`) avec chaîne de confiance (§3).
- **Trois langues** dès la V1 : anglais, français, allemand (application et certificat).

**Reporté (V2+)**
- Windows.
- ISO Live (orientation **Debian** à la place d'Alpine, à déterminer).
- SAS/SCSI, ponts USB, clés USB / cartes SD.
- TCG Opal / PSID revert.
- Effacement multi-disques en parallèle (prérequis de l'offre Atelier/ITAD).

**Exclu** : macOS.

## 2. Architecture des processus

```
┌─────────────────────────┐   pipe privé stdin/stdout, JSON    ┌──────────────────────────┐
│  oblivio (UI Tauri)     │  ◄──────────────────────────────►  │  oblivio-helper (root)   │
│  utilisateur normal     │   requêtes / réponses / événements │  lancé via pkexec,       │
│  WebView + React        │                                    │  1 fois par session      │
│  rendu PDF, traductions │                                    │  ioctl, moteur, signature│
└─────────────────────────┘                                    └──────────────────────────┘
```

- L'UI ne touche **jamais** un périphérique : elle demande, affiche, confirme, rend le PDF.
- Le helper est lancé **une fois par session** via `pkexec` et vit tant que l'UI est ouverte.
  Le moteur de décision, les protections et la signature tournent **dans le helper**.
- Protocole détaillé : [protocole_ipc.md](protocole_ipc.md).

### Workspace Rust

| Crate | Rôle | Accès disque | Dépôt |
|---|---|---|---|
| `oblivio-attestation` | Schéma de l'attestation, CBOR/COSE, vérification | Non | **Public** |
| `oblivio-verify` | CLI de vérification | Non | **Public** |
| `oblivio-protocol` | Types des messages UI ↔ helper, version du protocole | Non | Privé |
| `oblivio-core` | Modèles, moteur de décision, règles, machine d'états | Non (100 % testable) | Privé |
| `oblivio-device` | ioctl NVMe/ATA, SG_IO, O_DIRECT (buffers alignés), sysfs | Oui | Privé |
| `oblivio-license` | Parsing/vérification `.lic`, clé du poste, activation | Non | Privé |
| `oblivio-helper` | Binaire root : core + device + license + signature | Oui | Privé |
| `oblivio-report` | Rendu PDF/A-3, QR, traductions (Fluent) | Non | Privé |
| `oblivio-app` | Shell Tauri + frontend React | Non | Privé |

## 3. Modèle de confiance du certificat

```
Clé racine Oblivio (offline, éditeur)
   └── signe ──► Licence .lic  { client, offre, validité, clé publique du poste }
                     └── clé privée du poste (générée localement, jamais exportée)
                            └── signe ──► Payload d'audit canonique
                                              ├── rendu dans le PDF (lisible)
                                              └── encodé dans le QR (payload + signature + licence)
```

- On signe un **payload canonique** (CBOR ou JSON canonique), jamais le fichier PDF. Le PDF n'est qu'un rendu.
- Vérification offline avec la seule **clé publique racine** (vérificateur gratuit).
- Une clé de poste compromise n'affecte qu'une licence ; révocation via liste livrée avec les mises à jour.

## 4. États d'un disque

| État | Couleur | Signification | Action |
|---|---|---|---|
| **PRÊT** | Vert `#10B981` | Sain, méthode conforme identifiée | Purge / Clear |
| **PROTÉGÉ** | Gris | Disque système, monté, membre d'un volume | Aucune |
| **BLOQUÉ** | Orange (à ajouter à la charte) | ATA *Frozen* / *Locked*, commande indisponible | Action opérateur guidée (veille S3, mot de passe…) |
| **SAY NO** | Rouge `#EF4444` | Effacement impossible ou non prouvable | Destruction prescrite ou forçage (§5.3) |

*Frozen* et *Locked* ne déclenchent **jamais** une prescription de destruction.

Un disque **défaillant mais effaçable** reste PRÊT : l'effacement est certifié, et le certificat porte
séparément une recommandation de devenir « Destruction physique » (voir moteur_decision.md §3).

## 5. Règles de certification invariantes

1. **Une écriture hôte n'est jamais un Purge.** Seules les commandes firmware (NVMe Sanitize/Format, ATA Sanitize, ATA Secure Erase) produisent un certificat *Purge*.
2. **Validation propre à chaque méthode** :
   - Overwrite → contrôle du motif attendu.
   - Crypto Erase / Block Erase → comparaison d'échantillons hachés avant/après.
3. **Forçage d'un disque Say NO** : justification obligatoire, niveau plafonné à *Clear*,
   certificat marqué **« NON CONFORME — Destruction physique requise »** avec la justification.
   Ne peut jamais afficher *Purge* ni *Clear conforme*.
4. **Confirmation liée au disque** : saisie des 4 derniers **caractères** du numéro de série. Pas de mot-clé fixe.
5. **Tout refus produit un rapport signé** (rapport de refus).
6. **Traçabilité des sources** : chaque donnée du rapport indique sa provenance (firmware / source secondaire).

## 6. Points ouverts

- Seuils et règles du moteur → validés, reste 3 points IEEE 2883, voir [moteur_decision.md](moteur_decision.md).
- Format du certificat → proposé (CBOR + COSE_Sign1 + QR Base45, PDF/A-3), voir [certificat.md](certificat.md).
- ~~Bibliothèque PDF Rust~~ → `typst` (2026-10-04, voir [certificat.md](certificat.md) §6).
- Modèle de licence compatible Live USB (licence liée à l'opérateur ou au support, pas au poste).
- Distribution Debian pour l'ISO.

## 7. Langues (décidé le 2026-10-03)

- Application et certificat en **EN / FR / DE**. Langue par défaut : celle du système, modifiable.
- L'attestation signée ne contient **aucun texte traduit** : uniquement des codes (statuts, méthodes, N/D/R…).
  Un certificat émis en français peut donc être vérifié et affiché en allemand ou en anglais.
- Termes normatifs **non traduits** : *Clear*, *Purge*, *Destruct*, NIST SP 800-88, IEEE 2883.
- Le texte libre saisi (justification de forçage, organisation) est conservé tel quel.
- Outils : `react-i18next` côté interface, Project Fluent (`fluent-rs`) côté Rust pour le PDF.

## 8. Vérificateur open source (décidé le 2026-10-03)

- Dépôt public séparé, ne contenant **que** :
  - `oblivio-attestation` : schéma de l'attestation, décodage QR/CBOR/COSE, vérification de signature et de chaîne ;
  - `oblivio-verify` : CLI de vérification ;
  - la clé publique racine et les libellés des codes (EN/FR/DE).
- Le dépôt privé dépend de `oblivio-attestation` ; l'inverse est interdit.
- Rien du moteur de décision, des seuils, de l'accès disque, du helper ni de la génération de licences
  n'est publié. Ce qui est public l'est déjà de fait : tout ce que contient un QR est lisible par quiconque le scanne.
- Licence proposée : Apache-2.0.

## 9. smartmontools (décidé le 2026-10-03)

- **Jamais dans le chemin de décision.** Les capacités (Sanitize, Crypto Erase, Security) viennent de
  l'IDENTIFY firmware (NVMe Identify Controller `SANICAP`, ATA IDENTIFY), lu par `oblivio-device`.
- **Usage 1 — oracle de test (dev)** : tests différentiels entre notre parsing et `smartctl --json -x`
  sur le banc de disques.
- **Usage 2 — avis secondaire (runtime, optionnel)** : si `smartctl` est présent, lecture seule
  `smartctl --json -x` ; divergences affichées et consignées comme « source secondaire ».
  Absent → aucune dégradation.
- **Pas d'embarquement de `drivedb.h`** dans le binaire (GPL-2.0) ; lecture éventuelle du fichier installé sur le système.
- Base de quirks propre à Oblivio (firmwares connus défaillants sur Sanitize, etc.), versionnée et signée.
