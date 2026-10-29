# disphas — Cadre du projet (v0.1)

> Document de référence. Il prime sur les autres specs en cas de contradiction.
> Validé le 2026-10-03. Licences en ligne (§1, §2, §3, §6) : décidé le 2026-10-11. Offres (§1, ISO et
> parallèle) : décidé le 2026-10-20 ([offres](offres.md)).
>
> **Nom du produit : disphas** (décidé le 2026-10-13). Ancien nom : Oblivio. Le nom s'écrit en minuscules ;
> identité visuelle livrée par le graphiste (dossier de marque `~/Documents/graphism_Disphas/`, hors dépôt ;
> logos copiés dans `assets/logo/disphas/`, police Klaudia dans `assets/fonts/klaudia/`). Gardent l'ancien nom :
> les documents d'origine (§1 de `CLAUDE.md`, point 7), les logos `assets/logo/oblivio-*` (encore utilisés par le
> code, remplacés au lot 3 de [offres](offres.md)) et les domaines `oblivio.eu`.

## 1. Périmètre V1

**Inclus**
- Application desktop **Linux 64-bit** (Tauri v2 + moteur Rust).
- Supports **NVMe** et **SATA** en connexion directe (carte mère).
- Parcours complet : inspection → décision → exécution → vérification → validation → certificat PDF signé.
- Licences (`.lic`) avec chaîne de confiance (§3). Activation hors ligne par copier-coller (implémentée) ;
  **activation et renouvellement en ligne**, contrôle du nombre de postes, vente en ligne (décidé le 2026-10-11,
  non implémenté) : [licences_en_ligne.md](licences_en_ligne.md).
- **Trois langues** dès la V1 : anglais, français, allemand (application et certificat).
- **Effacement de plusieurs disques en parallèle**, en lots : fait et testé sur le banc (2026-10-06) ; nombre
  maximal de disques en parallèle selon l'offre ([offres](offres.md) §4, décidé le 2026-10-20, non implémenté).
- **ISO de démarrage** (orientation **Debian**, USB et réseau) : **avant le lancement** (décidé le 2026-10-20,
  [offres](offres.md) O2 et lot 5) ; document d'architecture d'abord, soumis au propriétaire.

**À terme** (décidé le 2026-10-20, [offres](offres.md) O10 : hors lancement, ne rien faire qui l'empêche)
- Windows.
- Effacement des clés USB, depuis l'ISO (« l'USB par l'ISO », O10 ; le démarrage de l'ISO sur clé USB, lui,
  fait partie du lot 5).

**Reporté (V2+)**
- SAS/SCSI, ponts USB, cartes SD.
- TCG Opal / PSID revert.

**Exclu** : macOS.

## 2. Architecture des processus

```
┌─────────────────────────┐   pipe privé stdin/stdout, JSON    ┌──────────────────────────┐
│  disphas (UI Tauri)     │  ◄──────────────────────────────►  │  disphas-helper (root)   │
│  utilisateur normal     │   requêtes / réponses / événements │  lancé via pkexec,       │
│  WebView + React        │                                    │  1 fois par session      │
│  rendu PDF, traductions │                                    │  ioctl, moteur, signature│
└─────────────────────────┘                                    └──────────────────────────┘
```

- L'UI ne touche **jamais** un périphérique : elle demande, affiche, confirme, rend le PDF.
- Le helper est lancé **une fois par session** via `pkexec` et vit tant que l'UI est ouverte.
  Le moteur de décision, les protections et la signature tournent **dans le helper**.
- Protocole détaillé : [protocole_ipc.md](protocole_ipc.md).
- **Réseau** (décidé le 2026-10-11, non implémenté) : seule la partie Rust de l'application (compte utilisateur)
  peut contacter le serveur de licences, pour activer ou renouveler la licence ; jamais la WebView (CSP inchangée),
  jamais le helper. Le helper ne reçoit que des textes `DSA1`/`DSL1` par les commandes existantes
  ([licences_en_ligne.md](licences_en_ligne.md) §5).
- **Côté éditeur** : site (comptes, abonnements, sièges) et **service de signature** isolé, seul détenteur de la
  clé boutique ([licences_en_ligne.md](licences_en_ligne.md) §6).

### Workspace Rust

| Crate | Rôle | Accès disque | Dépôt |
|---|---|---|---|
| `disphas-attestation` | Schéma de l'attestation, CBOR/COSE, vérification | Non | **Public** |
| `disphas-verify` | CLI de vérification | Non | **Public** |
| `disphas-protocol` | Types des messages UI ↔ helper, version du protocole | Non | Privé |
| `disphas-core` | Modèles, moteur de décision, règles, machine d'états | Non (100 % testable) | Privé |
| `disphas-device` | ioctl NVMe/ATA, SG_IO, O_DIRECT (buffers alignés), sysfs | Oui | Privé |
| `disphas-license` | Parsing/vérification `.lic`, clé du poste, activation | Non | Privé |
| `disphas-helper` | Binaire root : core + device + license + signature | Oui | Privé |
| `disphas-report` | Rendu PDF/A-3, QR, traductions (Fluent) | Non | Privé |
| `disphas-app` | Shell Tauri + frontend React | Non | Privé |
| `disphas-issuer` (`tools/`) | Outil de l'éditeur : clé racine, licences, révocations, certification de la clé boutique. Jamais livré | Non | Privé |
| `disphas-signer` (`tools/`) | Service de signature du serveur de licences : clés boutique, certificats `DSI1`, registre, socket locale ([licences_en_ligne.md](licences_en_ligne.md) §6). Jamais livré aux clients | Non | Privé |
| `disphas-fuzz` (`fuzz/`) | Cibles de fuzzing (cargo-fuzz), hors workspace | Non | Privé |

### Stack technique (état au 2026-10-04)

Versions relevées dans `Cargo.lock` et `app/package-lock.json` ; ce sont elles qui font foi.

| Domaine | Choix | Version | Où |
|---|---|---|---|
| Langage | Rust stable, édition 2024 | 1.99 (MSRV 1.85 ; helper 1.89 ; verify 1.88 ; report et app 1.92) | tout le workspace |
| Shell desktop | Tauri v2 (`tauri`, `@tauri-apps/api`, `@tauri-apps/cli`) | 2.11 / 2.11 / 2.12 | `app/` |
| Frontend | React + TypeScript, Vite | React 19.3, TS 6.0, Vite 8.3 | `app/src` |
| Style, icônes | Tailwind CSS 4 (plugin Vite), Lucide | 4.3, lucide-react 1.51 | `app/src` |
| Traductions UI | i18next + react-i18next (FR, EN, DE) | 26.4 / 17.0 | `app/src/i18n` |
| Lint frontend | oxlint | 1.86 | `app/` |
| Types partagés UI ↔ helper | ts-rs (génère `app/src/bindings`) | 12.0 | `disphas-protocol` |
| Sérialisation, erreurs | serde, serde_json ; thiserror | 1.0 ; 2.0 | partout |
| Attestation | CBOR (`ciborium`), COSE_Sign1 (`coset`), Ed25519 (`ed25519-dalek`), SHA-256 (`sha2`), zlib (`miniz_oxide`) | 0.2 ; 0.4 ; 3.0 ; 0.11 ; 0.9 | `disphas-attestation` |
| Lecture PDF (option `pdf`) | `lopdf` (MIT) | 0.45 | `disphas-attestation` |
| Rendu du certificat | Typst (PDF/A-3b), Fluent (traductions), `qrcode` | 0.15 ; fluent-bundle 0.16 ; 0.14 | `disphas-report` |
| Accès disques | ioctl directs via `libc` (NVMe admin, SG_IO / ATA PASS-THROUGH), sysfs | libc 0.2 | `disphas-device` |
| Secrets | `zeroize`, aléa de l'OS (`getrandom`) | 1.9 ; 0.4 | device, protocol, license |
| Clé racine (éditeur) | Argon2id + ChaCha20-Poly1305 | argon2 0.6 ; 0.11 | `tools/disphas-issuer` |
| Fuzzing | cargo-fuzz + libFuzzer, toolchain nightly | cargo-fuzz 0.13 | `fuzz/` |
| Node.js (développement) | via fnm | 24 | `app/` |
| CI | GitHub Actions (dépôt privé) : contrôles à chaque push, fuzzing hebdomadaire | — | `.github/workflows/` |

Écart avec les documents d'origine : **shadcn/ui n'est pas utilisé** ; les composants sont écrits à la main
(`app/src/components/ui.tsx`) avec Tailwind. Pas de gestionnaire d'état ni de routeur : l'état vit dans
`App.tsx`, le helper étant la seule source de vérité.

## 3. Modèle de confiance du certificat

```
Clé racine disphas (offline, éditeur)
   ├── signe ──► Licence v1 .lic  { client, offre, validité, clé publique du poste }      (implémenté)
   └── signe ──► Certificat d'émetteur { clé boutique, validité 1 an, offres, durée max }  (décidé le 2026-10-11)
                     └── clé boutique (serveur) ── signe ──► Licence v2 { …, certificat d'émetteur }
Licence v1 ou v2
   └── clé privée du poste (générée localement, jamais exportée)
          └── signe ──► Payload d'audit canonique
                            ├── rendu dans le PDF (lisible)
                            └── encodé dans le QR (payload + signature + licence)
```

- On signe un **payload canonique** (CBOR ou JSON canonique), jamais le fichier PDF. Le PDF n'est qu'un rendu.
- Vérification offline avec la seule **clé publique racine** (vérificateur gratuit) : la licence v2 embarque son
  certificat d'émetteur.
- Une clé de poste compromise n'affecte qu'une licence ; révocation via liste livrée avec les mises à jour.
- La **clé racine ne touche jamais le réseau**. La clé boutique, sur le serveur, peut être révoquée sans toucher
  à la racine ; les licences v1 restent acceptées. Détail, risques et limites : [licences_en_ligne.md](licences_en_ligne.md).

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
   L'application propose quatre motifs courants en un clic (demande du client, avant destruction, inspection
   impossible, responsable sécurité ; revue UX, 2026-10-29) : ils préremplissent le texte dans la langue de
   l'application, que l'opérateur peut compléter. Le texte reste libre et signé tel quel ; aucun code n'est ajouté
   à l'attestation.
   Ne peut jamais afficher *Purge* ni *Clear conforme*.
4. **Confirmation d'un effacement** (révisé le 2026-10-26, décision du propriétaire, revue UX) : une fenêtre
   met en garde (irréversible, disque, n° de série, méthode, avertissements) et se valide **d'un clic** ; plus de
   saisie. **Mode strict** en option (Paramètres) : le numéro de série lu sur le disque est exigé en plus, scanné
   en entier (douchette code-barres) ou ses 4 derniers **caractères** tapés. Le forçage d'un disque Say NO
   demande en plus sa justification (règle 3). La saisie n'a jamais été une barrière de sécurité (le n° de série
   est affiché) : elle protégeait d'une erreur de disque, ce que font la mise en garde et le mode strict. Un
   délai d'annulation de 10 s sans fenêtre a été essayé puis écarté par le propriétaire le même jour.
5. **Tout refus produit un rapport signé** (rapport de refus).
6. **Traçabilité des sources** : chaque donnée du rapport indique sa provenance (firmware / source secondaire).

## 6. Points ouverts

- Seuils et règles du moteur → validés, reste 3 points IEEE 2883, voir [moteur_decision.md](moteur_decision.md).
- Format du certificat → proposé (CBOR + COSE_Sign1 + QR Base45, PDF/A-3), voir [certificat.md](certificat.md).
- ~~Bibliothèque PDF Rust~~ → `typst` (2026-10-04, voir [certificat.md](certificat.md) §6).
- Modèle de licence compatible Live USB (licence liée à l'opérateur ou au support, pas au poste). La V1 garde
  la licence liée au poste ([licences_en_ligne.md](licences_en_ligne.md), D3, 2026-10-11) ; à traiter avec l'ISO.
- Licences en ligne ([licences_en_ligne.md](licences_en_ligne.md) §8) : tout est décidé (serveur
  `licence.oblivio.eu`, QR imprimé à 5,5 cm, 2026-10-11) ; reste un essai d'impression du QR le plus long.
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
  - `disphas-attestation` : schéma de l'attestation, décodage QR/CBOR/COSE, vérification de signature et de chaîne ;
  - `disphas-verify` : CLI de vérification ;
  - la clé publique racine et les libellés des codes (EN/FR/DE).
- Le dépôt privé dépend de `disphas-attestation` ; l'inverse est interdit.
- Rien du moteur de décision, des seuils, de l'accès disque, du helper ni de la génération de licences
  n'est publié. Ce qui est public l'est déjà de fait : tout ce que contient un QR est lisible par quiconque le scanne.
- Licence proposée : Apache-2.0.

## 9. smartmontools (décidé le 2026-10-03)

- **Jamais dans le chemin de décision.** Les capacités (Sanitize, Crypto Erase, Security) viennent de
  l'IDENTIFY firmware (NVMe Identify Controller `SANICAP`, ATA IDENTIFY), lu par `disphas-device`.
- **Usage 1 — oracle de test (dev)** : tests différentiels entre notre parsing et `smartctl --json -x`
  sur le banc de disques.
- **Usage 2 — avis secondaire (runtime, optionnel)** : si `smartctl` est présent, lecture seule
  `smartctl --json -x` ; divergences affichées et consignées comme « source secondaire ».
  Absent → aucune dégradation.
- **Pas d'embarquement de `drivedb.h`** dans le binaire (GPL-2.0) ; lecture éventuelle du fichier installé sur le système.
- Base de quirks propre à disphas (firmwares connus défaillants sur Sanitize, etc.), versionnée et signée.
