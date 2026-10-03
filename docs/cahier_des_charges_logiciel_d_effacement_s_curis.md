# Cahier des Charges Technique et Fonctionnel (Spécifications v2.0)

## Solution Intelligente de Sanitisation de Supports de Stockage et d'Audit de Conformité

## 1. Vision, Cadre Normatif & Positionnement

### 1.1 Objet du Document

Le présent document définit les spécifications fonctionnelles, techniques, réglementaires et d'expérience utilisateur (UX) pour la création d'un logiciel professionnel de décision et d'exécution de sanitisation de données, développé en **Rust** (backend) et **Tauri** (frontend GUI).

### 1.2 Alignement Normatif Strict (Mise à Jour Septembre 2025)

Le logiciel s'aligne rigoureusement sur le cadre normatif international et européen révisé :

* **NIST SP 800-88 Rev. 2 (Septembre 2025) :** Cadre directeur pour la gestion de la sanitisation des médias, la gouvernance, les exigences du certificat d'audit et la chaîne d'assurance.

* **IEEE 2883-2022 :** Standard technique de référence pour l'exécution des opérations matérielles et logiques de sanitisation (*Clear*, *Purge*, *Destruct*) couvrant les technologies HDD, SSD (SATA/NVMe), NVMe Namespaces, ZNS et SMR.

* **ISO/IEC 27040:2024 & ISO/IEC 19790 :** Sécurité du stockage et critères de remise à zéro (*zeroization*) des clés cryptographiques.

* **DIN 66399 / ISO/IEC 21964 :** Référentiel de classification pour la destruction physique des supports.

* **Conformité RGPD (Art. 17 & 32) & Directive NIS 2 :** Garanties de destruction irréversible sans fuite de données et traçabilité pour les entités essentielles/importantes.

### 1.3 Principes Directeurs Produit

1. **Décision Automatisée ("No-Brainer UX") :** Masquer la complexité technique sous un moteur d'évaluation qui recommande une unique action conforme.

2. **Capacité de Refus ("Say NO Policy") :** Si le support est défaillant, verrouillé ou hors d'état de garantir une sanitisation intégrale, le logiciel **refuse l'effacement logiciel** et prescrit formellement la destruction physique (*Destruct*).

3. **Souveraineté & Offline-First :** Aucune dépendance au cloud, aucun envoi de télémétrie, exécution 100 % autonome adaptée aux environnements isolés (*air-gapped*).

4. **Le Certificat PDF comme Produit d'Audit :** Génération d'un rapport de conformité inviolable, signé cryptographiquement.

## 2. Architecture Technique & Interactions Système

### 2.1 Stack Technologique

* **Cœur / Backend :** Rust 1.80+ (Sécurité mémoire, zéro surcoût à l'exécution, gestion fine du matériel).

* **Interface Graphique (GUI) :** Tauri v2 (HTML5/CSS3/TypeScript via Webview native, empreinte RAM < 40 Mo).

* **Communication Interne :** Commandes typées inter-processus Tauri (`tauri::command`) et flux d'événements asynchrones (*IPC Streaming*).

* **Plateforme Cible Principale :** Linux 64-bit (OS hôte & Live USB autonome).

* **Portage Secondaire :** Microsoft Windows 10/11 & Server (via abstractions Win32 Native storage I/O).

* **Hors Scope :** Apple macOS.

### 2.2 Direct Storage I/O (Absence de Dépendances Externe)

* **Interdiction des Wrappers CLI :** Le binaire Rust ne doit utiliser aucun outil système tiers (`hdparm`, `nvme-cli`, `smartctl`, `dd`, `diskpart`).

* **Invocations Kernel Natives (Linux) :**

  * Ouverture directe des périphériques block via `std::fs::OpenOptions` (`/dev/sdX`, `/dev/nvmeXnX`).

  * Bypass du cache du noyau Linux via le flag `O_DIRECT`.

  * Interactions bas niveau via appels `ioctl` directs :

    * **NVMe :** `NVME_IOCTL_ADMIN_CMD` (Sanitize, Format, Identify, Get Log Page).

    * **ATA/SATA :** Pass-through SG_IO (`HDIO_DRIVE_CMD`, `ATA_PASSTHROUGH_16`) pour émettre les commandes *ATA Secure Erase* / *Crypto Erase*.

    * **SCSI/SAS :** Commandes `SANITIZE` (Block Erase, Overwrite, Crypto Erase).

    * **Block Layer :** Callbacks `BLKDISCARD`, `BLKSECDISCARD`, `BLKZEROOUT`.

### 2.3 Élévation de Privilèges & Sécurité Système

* Gestion native de l'élévation d'autorisations (Polkit / `pkexec` sous Linux, Manifest UAC sous Windows).

* Protection contre la destruction du système hôte : Isolation stricte du disque contenant la racine du système d'exploitation (`/` ou `C:`).

## 3. Moteur de Décision & Qualification Matérielle

### 3.1 Étape 1 : Inspection Intelligente du Support

Le logiciel interroge directement le contrôleur du disque pour construire un profil matériel complet :

1. **Identité Matérielle :** Constructeur, Modèle exact, Numéro de Série (SN), Version de Firmware, Bus/Interface (NVMe, SATA, SAS, USB, PCIe).

2. **Topologie de Stockage :** Taille des secteurs (512n, 512e, 4Kn), capacité exacte en octets, présence de namespaces NVMe multiples ou de zones SMR/ZNS.

3. **Analyse des Zones Masquées & Protections :**

   * Détection des zones HPA (*Host Protected Area*) et DCO (*Device Configuration Overlay*).

   * Vérification de l'état du mot de passe ATA Security (*Frozen*, *Locked*, *Unlocked*).

4. **Bilan de Santé Health/S.M.A.R.T. :**

   * Nombre de secteurs réalloués (*Reallocated Sectors Count* / *Media Errors*).

   * Pourcentage d'usure des cellules (*Percentage Used* / *Wear Leveling*).

   * État du sous-système de réserve de blocs (*Available Spare*).

### 3.2 Étape 2 : Moteur de Recommandation Unique (IEEE 2883 / NIST Rev. 2)

Le logiciel évalue le support et applique l'arbre de décision suivant :

```
                        [INSPECTION DU SUPPORT]
                                   │
       ┌───────────────────────────┴───────────────────────────┐
       ▼                                                       ▼
[CRITÈRES D'ÉCHEC]                                  [SUPPORT FONCTIONNEL]
• Secteurs défectueux critiques                         │
• ATA Security State = FROZEN irrecevable               ├──► SSD NVMe / SATA / SED
• Firmware altéré / Erreur I/O critique                 │     └──► RECOMMANDATION : PURGE
• Média physique endommagé                              │          (NVMe Sanitize / ATA Secure Erase / CE)
       │                                                │
       ▼                                                ├──► HDD Magnétique
[RÉPONSE : REFUS D'EFFACEMENT LOGICIEL]                  │     └──► RECOMMANDATION : PURGE ou CLEAR
   Prescription formelle :                              │          (Passage unique / Write Zero - IEEE 2883)
   DESTRUCTION PHYSIQUE (IEEE 2883 Destruct)             │
                                                        └──► Clé USB / Carte SD (Flash Basique)
                                                              └──► RECOMMANDATION : CLEAR
                                                                   (Overwrite + Verification)

```

#### Élimination des Mythes Legacy

* **Pas de multi-passes obsolètes :** Conforme à NIST SP 800-88 Rev. 2, le logiciel supprime l'option DoD 5220.22-M (3 ou 7 passes) pour les supports modernes (SSD/NVMe et HDD récents), car un écrasement unique à zéro suffi pour le *Clear*, et les commandes firmware natives sont requises pour le *Purge*.

## 4. Workflow de Sanitisation & Assurance Duale

Conformément à la révision 2 du NIST SP 800-88, le processus de sanitisation est découpé en trois phases distinctes : **Exécution**, **Vérification** et **Validation**.

```
┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐
│   1. EXÉCUTION   │ ──► │  2. VÉRIFICATION │ ──► │   3. VALIDATION  │
│ (Command/I/O)    │     │  (Status Check)  │     │ (LBA Sampling)   │
└──────────────────┘     └──────────────────┘     └──────────────────┘

```

### 4.1 Phase 1 : Exécution

1. Démontage des volumes logiques et déverrouillage du disque (suppression des structures HPA/DCO si présentes).

2. Émission de la commande matérielle de Purge (*Cryptographic Erase* via ISO/IEC 19790 zeroization de la clé MEK, *Block Erase*, ou *NVMe Sanitize*).

3. En cas d'incompatibilité de la commande matérielle (ex: pont USB-SATA bon marché), bascule automatique (*fallback*) vers un écrasement logique séquentiel conforme à IEEE 2883 Clear.

### 4.2 Phase 2 : Vérification (Verification)

* Validation que la commande émise s'est terminée avec un code de retour du firmware de succès ($0x00$).

* Contrôle des registres de statut post-opération (ex: *NVMe Sanitize Status Log Page*).

### 4.3 Phase 3 : Validation (Validation)

* **Échantillonnage par LBA (Logical Block Address) :**

  * Inspection obligatoire des blocs : $0\%$ (Début), $50\%$ (Milieu), $100\%$ (Fin).

  * Contrôle aléatoire sur au moins 100 zones LBA distribuées sur la totalité de la surface du disque.

* **Contrôle d'Homogénéité :** Vérification que tous les octets lus dans les échantillons correspondent au motif attendu (ex. $0x00$).

* **Calcul de Hash de Contrôle :** Empreinte SHA-256 calculée sur l'échantillon.

## 5. Le Certificat et Rapport d'Audit PDF

Le rapport PDF généré est le livrable juridique attestant de la conformité au NIST SP 800-88 Rev. 2 et au RGPD.

### 5.1 Contenu Obligatoire du Rapport

* **Entête d'Audit :** Identifiant unique (UUID v4), Horodatage ISO-8601 UTC, Nom de l'organisation et de l'opérateur.

* **Information Support (Asset Metadata) :** Marque, Modèle, Numéro de Série (SN), Révision du Firmware, Capacité, Type de Média.

* **Méthode de Sanitisation :**

  * Norme appliquée : NIST SP 800-88 Rev. 2 / IEEE 2883-2022.

  * Catégorie atteinte : *Clear*, *Purge*, ou *Destruct Recommended*.

  * Technique spécifique : Ex. *NVMe Sanitize Crypto Erase (ISO/IEC 19790 Zeroization)*.

* **Résultats Distincts d'Assurance :**

  * Statut de Vérification : `SUCCÈS` / `ÉCHEC`.

  * Statut de Validation : `SUCCÈS (Échantillonnage 100 LBA conforme à 0x00)`.

* **Outillage :** Nom du logiciel, version exacte du binaire Rust, hash SHA-256 de l'exécutable.

### 5.2 Inviolabilité et Traçabilité Cryptographique

* **Signature Numérique Immédiate :** Chaque PDF généré est signé par le backend Rust avec une clé asymétrique **Ed25519** liée à la licence de l'utilisateur.

* **QR Code d'Authenticité Hors-Ligne :** Empreinte SHA-256 du rapport intégrée sous forme de QR Code sur le document, permettant de vérifier l'intégrité du document PDF sans connexion Internet.

## 6. Interface Utilisateur (UX / UI)

### 6.1 Principes d'Ergonomie

* **Carte d'Équipement Claire :** Les disques connectés apparaissent sous forme de blocs visuels indiquant la taille, le modèle et l'état.

* **Code Couleur de Sécurité :**

  * **VERT :** Disque prêt, sain, action recommandée identifiée.

  * **ORANGE :** Disque contenant l'OS actif / Disque protégé (Action bloquée).

  * **ROUGE :** Support défaillant / Secteurs critiques défectueux -> **Alerte : Destruction Physique Requise**.

* **Confirmation Anti-Erreur :** L'opérateur doit obligatoirement saisir une chaîne de caractères contenant les 4 derniers chiffres du numéro de série du disque (Ex: `CONFIRMER-1234`) pour lancer la procédure.

* **Moniteur d'Exécution :** Barre de progression fluide, vitesse de transfert en temps réel ($MB/s$), temps restant calculé et logs défilants.

## 7. Modèle de Licence & Architecture Offline-First

### 7.1 Fonctionnement 100 % Hors-Ligne

* L'application n'effectue aucun appel réseau pour valider une licence ou signer un rapport.

* Les licences sont fournies sous forme de fichiers signés cryptographiquement (`.lic`) contenant les droits d'utilisation.

### 7.2 Grille Tarifaire Disruptive (Face aux Modèles au Disque)

| Offre | Cible | Périmètre | Tarification | 
 | ----- | ----- | ----- | ----- | 
| **Freemium / Diagnostic** | Particuliers, Techs | Inspection, S.M.A.R.T., Recommandation NIST Rev. 2 | **Gratuit** | 
| **Licence Pro** | Indépendants, Techniciens IT | Effacement illimité sur 1 poste, Rapports PDF signés | **89 € / an** ou **149 € à vie** | 
| **Atelier / ITAD** | Reconditionneurs, MSP | Effacements illimités, personnalisation des rapports PDF avec logo | **79 € / mois / atelier** | 
| **Grands Comptes** | Administrations, OIV | Option Live USB bootable autonome, intégration sur-mesure | **Sur Devis** | 

## 8. Modalités de Déploiement : ISO Live-USB Minimaliste

Afin d'effacer les machines complètes destinées au réemploi sans démonter les disques :

* Création d'une image **ISO Bootable** ultra-légère basée sur Alpine Linux.

* Démarrage directement en mémoire RAM (Live OS).

* Lancement automatique de l'interface Tauri en mode Kiosque autonome (sans bureau Linux lourd).

* Exportation des rapports PDF signés vers une clé USB externe connectée.