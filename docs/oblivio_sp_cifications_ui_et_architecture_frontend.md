# Oblivio — Spécifications Interface Utilisateur & Architecture Frontend

Ce document définit la stratégie d'interface utilisateur (UI), la Direction Artistique (DA) ainsi que la pile technique retenue pour **Oblivio**, garantissant une ergonomie industrielle, une grande réactivité et une maintenance simplifiée.

## 1. Choix de la Stack Technique

L'objectif est d'allier un rendu visuel haut de gamme (style *Linear*, *Raycast*, *Tailscale*) avec une architecture robuste, légère et maintenable.

| Couche | Technologie | Rôle & Justification | 
 | ----- | ----- | ----- | 
| **Backend** | **Rust (Tauri v2)** | Accès bas niveau (`O_DIRECT`, `ioctl` NVMe/ATA), signature Ed25519, génération du rapport PDF. | 
| **Shell App** | **Tauri v2** | Lien IPC sécurisé entre Rust et le Webview, empreinte mémoire minimale (\~15-20 Mo). | 
| **Frontend** | **React + Vite** | Standard de l'industrie pour les interfaces riches, intégration fluide avec l'écosystème web moderne et réactivité optimale. | 
| **Styling** | **Tailwind CSS** | Design system utilitaire. Garantit la cohérence des marges, de la typographie et des couleurs. | 
| **Composants** | **Shadcn UI** | Composants pré-stylisés industriels modernes (basés sur Radix UI). Accessibles et facilement personnalisables. | 
| **Iconographie** | **Lucide React** | Set d'icônes vectorielles épurées, adaptées aux interfaces techniques et de cybersécurité. | 

## 2. Direction Artistique & System Design

L'UI privilégie une esthétique **sombre, industrielle, chirurgicale et zéro distraction**. Aucune fioriture superficielle.

### Palette de Couleurs

* **Fond Application (`bg-background`) :** `#090D16` (Bleu nuit très sombre, mat).

* **Conteneurs / Cartes (`bg-card`) :** `#111827` avec bordure fine `#1F2937` (`border-slate-800`).

* **Texte Principal :** `#F9FAFB` (Blanc cassé doux).

* **Texte Secondaire / Métadonnées :** `#9CA3AF` (Gris acier).

* **Code / Numéros de Série :** Polices monospacées (`JetBrains Mono`).

### Codes Couleur d'État

* **Vert Émeraude (`#10B981`) — OK / PURGE :** Disque conforme, décision *NIST Purge / Clear* prête.

* **Rouge Cerise (`#EF4444`) — SAY NO / DESTRUCT :** Disque défaillant ou secteur critique altéré. Recommandation de destruction physique.

* **Indigo (`#6366F1`) — ACTION / CERTIFICAT :** Boutons d'action principale et signature d'audit.

### Typographie

* **UI & Titres :** `Plus Jakarta Sans` ou `Inter` (Sans-serif moderne).

* **Données Techniques :** `JetBrains Mono` ou `Fira Code` (Nœuds système, S/N, hashes Ed25519).

## 3. Structure des Composants Visuels

### A. Carte Disque Saine (NIST Purge / Clear)

```
+-----------------------------------------------------------------------------------+
|  [NVMe]  Samsung SSD 980 PRO 1TB                                HEALTH: 99% [OK]  |
|  SN: S5GXNX0T1234567 | Bus: PCIe 4.0 x4 | Node: /dev/nvme0n1                      |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  | RECOMMENDATION OBLIVIO : NIST SP 800-88 Rev.2 PURGE                         |  |
|  | Méthode : NVMe Sanitize (Block Erase) + Verification native firmware        |  |
|  +-----------------------------------------------------------------------------+  |
|                                                                                   |
|  [ Info SMART ]  [ Examiner la table de partition ]       [ EXECUTER LA PURGE -> ]  |
+-----------------------------------------------------------------------------------+

```

### B. Carte Disque Défaillante (Politique "Say NO")

```
+-----------------------------------------------------------------------------------+
|  [HDD]  Seagate Barracuda 2TB                                   HEALTH: 12% [BAD] |
|  SN: W1F0ABCD | Bus: SATA III | Node: /dev/sdb                                    |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  | ! DECISION OBLIVIO : RECOMMANDATION DE DESTRUCTION PHYSIQUE (SAY NO)        |  |
|  | Raison : Secteurs réalloués critiques & échec des commandes SMART ioctl.   |  |
|  +-----------------------------------------------------------------------------+  |
|                                                                                   |
|  [ Voir le rapport de refus PDF ]                           [ FORCER L'EFFACEMENT ]|
+-----------------------------------------------------------------------------------+

```

## 4. Workflow Utilisateur en 4 Écrans

1. **Écran de Détection (Dashboard Home) :**

   * Liste automatique des disques sous forme de cartes Shadcn.

   * Indicateurs d'état système en haut à droite : `OFFLINE` (Pastille verte) / `PRIVILEGES ROOT` (Pastille indigo).

2. **Modal de Confirmation (Safety Guard) :**

   * Saisie explicite du mot-clé `PURGER` ou des 4 derniers caractères du numéro de série pour déverrouiller l'action.

3. **Écran d'Exécution (Monitoring En Cours) :**

   * Barre de progression fluide (`transition-all duration-300`).

   * Console d'audit rétractable affichant les logs IPC envoyés en temps réel par Rust.

4. **Écran de Clôture (Résultat & Certificat) :**

   * Badge de succès ou de refus irréversible.

   * Bouton d'exportation immédiate du **Certificat d'Audit PDF signé**.

## 5. Directives de Style & Cohérence Visuelle

Afin de maintenir l'intégrité visuelle lors du développement des composants :

* **Sobriété :** Privilégier les aplats sombres et les bordures fines (`slate-800`) plutôt que des dégradés complexes.

* **Lisibilité :** Utiliser la typographie monospace uniquement pour les données brutes (identifiants, numéros de série, chemins d'accès matériel).

* **Contraste d'Action :** Utiliser la couleur d'accent (Indigo) exclusivement sur les boutons d'action principale et les confirmations de sécurité.