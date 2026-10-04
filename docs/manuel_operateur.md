# Oblivio — Manuel de l'opérateur

Version 0.1 · octobre 2026 · Linux

Oblivio efface les disques durs et SSD (SATA et NVMe) d'un ordinateur selon NIST SP 800-88 Rev. 2 et
IEEE 2883-2022, et produit pour chaque disque un **certificat signé**, vérifiable hors ligne. Il ne
demande qu'une chose à l'opérateur : confirmer. La décision (effacer, avec quelle méthode, ou refuser)
est prise par Oblivio à partir de l'état réel du disque.

> ⚠️ Un effacement est **irréversible**. Vérifiez toujours le numéro de série du disque avant de confirmer.

---

## 1. Installation

### Systèmes pris en charge

| Paquet | Systèmes |
|---|---|
| `.deb` | Ubuntu 22.04 et plus récent, Debian 12 et plus récent, Linux Mint 21+, Pop!_OS 22.04+ |
| `.rpm` | Fedora, openSUSE |

Les disques doivent être branchés **en direct** (SATA ou M.2/NVMe). Les adaptateurs USB ne sont pas pris
en charge.

### Installer

Dans un terminal, depuis le dossier où se trouve le paquet :

```bash
sudo apt install ./Oblivio_0.1.0_amd64.deb                                   # Ubuntu, Debian, Mint, Pop!_OS
sudo dnf install --nogpgcheck ./Oblivio-0.1.0-1.x86_64.rpm                   # Fedora
sudo zypper install --allow-unsigned-rpm ./Oblivio-0.1.0-1.x86_64.rpm        # openSUSE
```

Le gestionnaire de paquets installe aussi tout ce dont Oblivio a besoin.

### Autoriser un opérateur

Les certificats sont réservés au groupe `oblivio`. Un administrateur y ajoute chaque compte opérateur :

```bash
sudo usermod -aG oblivio <compte>
```

L'opérateur doit ensuite **fermer puis rouvrir sa session**. Sans cela, l'onglet *Historique* affiche
« Accès aux certificats refusé ».

### Activer la licence (hors ligne)

Oblivio ne se connecte jamais à Internet. L'activation se fait par échange de textes :

1. Cliquez sur le badge de licence en haut à droite (« Sans licence »).
2. **1. Créer la demande d'activation**, puis **Copier**. Le texte ne contient que la clé publique du
   poste, aucune donnée personnelle.
3. Envoyez ce texte à Oblivio (courriel, clé USB…). Vous recevez une licence en retour.
4. **2. Coller la licence reçue**, puis **Installer la licence**. Le badge passe à « Sous licence ».

Sans licence valide, Oblivio inspecte les disques mais n'efface rien.

---

## 2. Lancer Oblivio

Ouvrez **Oblivio** depuis le menu des applications. Le système demande le **mot de passe d'un
administrateur** : il autorise le module d'effacement (qui travaille avec les droits root) pour cette
session uniquement. Il est redemandé à chaque lancement.

En haut de la fenêtre :

| Élément | Signification |
|---|---|
| HORS LIGNE | Oblivio ne communique avec aucun serveur |
| HELPER CONNECTÉ | Le module d'effacement est prêt. « HELPER INDISPONIBLE » : relancez Oblivio |
| Badge de licence | État de la licence ; un clic ouvre la fenêtre de licence |
| ⚙ (roue dentée) | **Réglages** (§ 7) |
| FR / EN / DE | Langue de l'interface |

---

## 3. L'écran Disques

Chaque disque branché apparaît avec son modèle, sa capacité, son **numéro de série** (S/N) et son état.
Oblivio l'inspecte automatiquement ; **Actualiser** relance l'inspection de tous les disques.

### Les quatre états

| État | Signification | Ce que vous pouvez faire |
|---|---|---|
| **PROTÉGÉ** | Oblivio ne touchera pas à ce disque : disque système, partition montée, swap actif, membre d'un volume (LVM, RAID, chiffrement) | Rien : c'est voulu. Pour effacer le disque système d'un ordinateur, branchez ce disque sur un **autre** poste Oblivio |
| **BLOQUÉ** | Le disque est sain, mais une action est nécessaire avant d'effacer | Suivez la consigne affichée (tableau ci-dessous), puis **Actualiser** |
| **SAY NO** | L'effacement de ce disque **n'est pas prouvable** | **Destruction physique** requise. Parfois, **Forcer l'effacement** est proposé (§ 4.4) |
| **PRÊT** | Oblivio peut effacer et prouver l'effacement | Choisir la méthode (la recommandée est cochée) et **Effacer** |

### Disque BLOQUÉ : que faire ?

| Code | Cause | Action |
|---|---|---|
| B1 | Sécurité ATA gelée (Frozen) | Mettez l'ordinateur en **veille**, réveillez-le, puis **Actualiser** |
| B2 | Disque verrouillé par un mot de passe ATA | **Déverrouiller…** (§ 8.1) |
| B3 | Verrou de gel Sanitize actif | Éteignez puis rallumez le disque (cycle d'alimentation) |
| B4 | Une opération Sanitize est en cours | Patientez : elle reprend automatiquement |
| B5 | L'opération Sanitize précédente a échoué | Destruction physique, ou remise en état avec l'outil du constructeur (§ 8.4) |
| B6 | Tentatives de mot de passe épuisées | Cycle d'alimentation du disque |
| B7 | Sécurité ATA activée par un tiers | Faites retirer le mot de passe par son propriétaire, puis **Actualiser** |

### Disque PRÊT : la décision Oblivio

- **Méthode recommandée** et autres méthodes autorisées, avec leur **catégorie** :
  - **Purge** : effacement par le firmware du disque, qui protège contre une récupération en
    laboratoire ;
  - **Clear** : réécriture par l'ordinateur, protège contre une relecture ordinaire. Une réécriture par
    l'ordinateur n'est **jamais** certifiée Purge.
- **Réserves** (codes R) : points d'attention à connaître, qui n'empêchent pas l'effacement.
- **Devenir recommandé** : « Réemploi », ou « destruction physique recommandée » si le disque est usé
  (codes D) — l'effacement sera certifié, mais le disque ne devrait pas être réutilisé.

La liste complète des codes est en annexe.

---

## 4. Effacer un disque

### 4.1 Lancer

1. Sur un disque **PRÊT**, choisissez la méthode (la recommandée par défaut), puis **Effacer — Purge**
   (ou Clear).
2. Dans la fenêtre **Confirmer l'effacement** :
   - vérifiez le modèle et la méthode, et la ligne « Signé au nom de » (opérateur et organisation, § 7) ;
   - cochez chaque avertissement (réserves, devenir recommandé, « Tous les namespaces du contrôleur NVMe
     seront effacés » le cas échéant) ;
   - **tapez les 4 derniers caractères du numéro de série**, lus **sur l'étiquette du disque** ;
   - pour une réécriture par l'ordinateur seulement : option **Terminer par une passe de zéros** (§ 4.3).
3. **Effacer définitivement.**

Plusieurs disques peuvent être effacés **en même temps** : lancez-les l'un après l'autre.

### 4.2 Suivre

L'onglet **Effacements** liste les effacements de la session ; chaque carte de disque affiche aussi sa
progression. Les étapes : Préparation → Canaris → Effacement → Vérification → Validation → Signature →
Terminé.

> ⚠️ Pendant un effacement : **ne débranchez pas le disque et n'éteignez pas l'ordinateur.**

À la fin, le statut s'affiche :

| Statut | Signification |
|---|---|
| **CONFORME** | L'effacement est prouvé et certifié |
| **NON CONFORME** | Effacement forcé (§ 4.4) : destruction physique requise |
| **REFUS** | L'effacement n'a pas pu être prouvé : destruction physique requise |

**Exporter le certificat PDF signé** enregistre le PDF dans `~/Documents/Oblivio` ; **Ouvrir** l'affiche.

### 4.3 Réécriture par l'ordinateur et passe de zéros

Quand le disque n'offre pas de commande d'effacement firmware utilisable, Oblivio réécrit chaque secteur
avec un flux pseudo-aléatoire (catégorie Clear, réserve R7). L'option **Terminer par une passe de zéros**
ajoute une seconde passe pour que le disque paraisse vierge : la durée double, les deux passes sont
vérifiées et le certificat l'indique.

### 4.4 Forcer l'effacement (SAY NO)

Quand l'inspection du disque est impossible (N1) mais qu'il accepte encore l'écriture, Oblivio propose
**Forcer l'effacement**. L'effacement est alors une réécriture par l'ordinateur, certifiée au plus Clear
et toujours **NON CONFORME** : la destruction physique reste requise. Une **justification** de
20 caractères au moins est obligatoire ; elle est signée telle quelle dans le certificat — n'y mettez
**aucune donnée personnelle**.

---

## 5. Lots (gros volumes)

Pour un parc de disques (une commande, un client) :

1. Onglet **Disques**, barre du haut : **Nouveau lot**, libellé facultatif (ex. « commande 4512 »),
   **Ouvrir le lot**. Le lot reçoit un numéro (`LOT-AAAAMMJJ-XXXX`).
2. Effacez les disques : chacun reçoit le numéro de lot **dans son certificat signé**. Avec un lot ouvert,
   vous restez sur la liste des disques après chaque lancement.
3. Quand tout est terminé : **Clore le lot**. Le **certificat de lot** (PDF) est produit aussitôt : une
   ligne par disque (S/N, WWN ou EUI-64, modèle, capacité, méthode, catégorie, statut), et les
   certificats individuels signés **joints** au PDF.

Un lot ne peut pas être clos tant qu'un de ses effacements est en cours. Le certificat de lot peut être
régénéré plus tard depuis l'**Historique**.

---

## 6. Certificats

### 6.1 Historique

L'onglet **Historique** liste tous les certificats du poste, du plus récent au plus ancien. Chacun est
**revérifié** à l'affichage : un certificat altéré apparaît en rouge, sans son contenu.

- **Rechercher** par modèle, S/N, opérateur ; filtrer par **lot** ou par date (**Antérieurs au**).
- **Certificat PDF** : régénère le PDF dans la langue de l'interface.
- **Certificat de lot (PDF)** : quand un lot est sélectionné.
- **Supprimer les certificats affichés** : pour appliquer votre durée de conservation. Suppression
  définitive, inscrite dans un journal des suppressions ; les PDF déjà exportés ne sont pas touchés.

### 6.2 Vérifier un certificat

Onglet **Vérifier** : déposez un PDF (individuel ou de lot) ou un fichier `.cose`, ou collez le texte du
QR code (`OBV1:…`). Le résultat indique **Certificat authentique** ou **NON authentique** avec la raison.
Le PDF n'est qu'une mise en page : c'est l'attestation signée **jointe** qui est vérifiée.

Vos clients pourront aussi vérifier sans Oblivio, avec l'outil libre et gratuit `oblivio-verify`
(publication prévue) :

```bash
oblivio-verify certificat.pdf
```

---

## 7. Réglages

Roue dentée ⚙ en haut à droite :

- **Organisation** : signée dans chaque certificat (ex. le client ou votre atelier).
- **Opérateur** : un **identifiant** suffit (ex. `OP-042`) — le certificat ne pourra plus être modifié.
- **Logo sur les certificats** : PNG ou SVG, 1 Mo au plus ; remplace le logo Oblivio en tête des PDF.

Ces réglages valent pour le compte utilisateur.

---

## 8. Situations particulières

### 8.1 Disque verrouillé par un mot de passe (B2)

**Déverrouiller…**, choisissez **Utilisateur** ou **Maître**, saisissez le **mot de passe ATA** (celui
posé dans le disque par son ancien propriétaire, pas votre mot de passe Linux). Oblivio déverrouille le
disque et retire le mot de passe. **Chaque essai raté compte** : après 5 échecs, le disque refuse tout
essai jusqu'à ce qu'il soit éteint puis rallumé (B6). Le mot de passe maître ne fonctionne que si le
niveau de sécurité du disque est « High ».

### 8.2 Coupure de courant ou arrêt pendant un effacement

Au lancement suivant :
- un **effacement firmware (Sanitize)** se poursuit tout seul dans le disque : Oblivio le reprend,
  le vérifie et le certifie (onglet **Effacements**, mention « repris après un arrêt inattendu ») ;
- les autres effacements apparaissent en rouge, **« Effacement interrompu, sans certificat »** :
  relancez l'inspection puis l'effacement du disque.

### 8.3 Disque système

Oblivio refuse toujours d'effacer le disque sur lequel il tourne. Démontez le disque et effacez-le
depuis un autre poste.

### 8.4 Sanitize en échec (B5)

Le disque signale que sa dernière opération Sanitize a échoué. Cet état **persiste** même après un
redémarrage : il faut une commande spécifique (« Exit Failure Mode ») que cette version d'Oblivio ne
propose pas encore. Traitez le disque en destruction physique, ou faites-le remettre en état avec l'outil
de son constructeur avant de relancer l'inspection.

---

## 9. Désinstaller

```bash
sudo apt remove oblivio    # garde la clé du poste, la licence et les certificats
sudo apt purge oblivio     # efface TOUT : /var/lib/oblivio et le groupe oblivio
```

Sur Fedora / openSUSE, la désinstallation garde les données ; pour tout effacer :
`sudo rm -rf /var/lib/oblivio && sudo groupdel oblivio`.

Les PDF exportés dans `~/Documents/Oblivio` vous appartiennent et ne sont jamais supprimés par la
désinstallation. Avant `purge`, conservez les certificats dont vous avez besoin : ce sont vos preuves.

---

## 10. Sécurité

- Oblivio ne se connecte à aucun serveur et n'envoie aucune donnée.
- Une faille de sécurité ? Voir [SECURITY.md](../SECURITY.md).

---

## Annexe — Codes

**Protection (P)** : P1 disque système (/, /boot ou EFI) · P2 une partition est montée · P3 une partition
est en swap actif · P4 membre d'un volume (LVM, RAID, chiffrement) · P5 un autre namespace du même
contrôleur est protégé.

**Blocage (B)** : voir § 3.

**Refus — SAY NO (N)** : N1 inspection du disque impossible · N3 support en lecture seule · N8 erreurs de
lecture pendant la validation · N9 aucune méthode exécutable · N10 vérification ou validation en échec.

**Devenir — destruction recommandée (D)** : D1 le firmware signale une fiabilité dégradée · D2 sauvegarde
de la mémoire volatile en échec · D3 le S.M.A.R.T. prédit une panne · D4 nombre élevé de secteurs
réalloués · D5 secteurs réalloués pendant l'effacement.

**Réserves (R)** : R1 réserve de cellules sous le seuil · R2 endurance nominale épuisée · R3 erreurs
d'intégrité du support enregistrées · R4 quelques secteurs réalloués · R5 secteurs en attente ou
incorrigibles · R6 erreurs non corrigées signalées · R7 écriture hôte : secteurs réalloués non couverts ·
R8 modèle répertorié dans la base d'anomalies Oblivio.

**Méthodes** : NVMe Sanitize (Block Erase, Crypto Erase) · NVMe Format (effacement cryptographique, effacement
des données) · ATA Sanitize (Block Erase, Crypto Scramble, Overwrite) · ATA Security Erase (renforcé, normal)
· Réécriture hôte, une passe d'un flux pseudo-aléatoire.
