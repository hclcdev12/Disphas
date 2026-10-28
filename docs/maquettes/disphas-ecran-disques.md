# Disphas · Écran Disques — spécification UI

> À destination de Claude Code. Objectif : reproduire fidèlement cet écran dans l'application, en responsive.
> Référence visuelle et comportementale : `disphas-ecran-disques.html` (HTML/CSS/JS sans dépendance, à ouvrir dans un navigateur, redimensionner pour voir les trois tailles).
> Périmètre : **uniquement l'onglet Disques**. Ne pas modifier les autres onglets (Effacements, Historique, Vérifier) au-delà de l'en-tête partagé.

---

## 1. Principe de l'écran

Disposition **maître / détail** :

- **À gauche, la liste** des disques détectés, groupés par ce que l'opérateur doit faire : À lancer, À débloquer, Refusés, Protégés.
- **À droite, le détail** du disque sélectionné :
  - pour un disque prêt, la décision de Disphas (méthode d'effacement) et le bouton **Effacer** ;
  - pour les autres états, l'explication et l'action possible.

Règles de la charte appliquées (Gestalt) :

| Principe | Application |
|---|---|
| Proximité | Le disque, son état et son action sont sur une même ligne ; les groupes sont séparés par 20 px. |
| Région commune | La décision, les avertissements et l'explication sont chacun dans un cadre ; la ligne sélectionnée prend un fond. |
| Similarité | Une couleur et une icône par état, identiques dans la liste (titre de groupe, bouton) et dans le détail (bouton). Les cases à cocher et les boutons radio forment une seule famille : même taille, même contour, même couleur quand ils sont choisis. |
| Figure / fond | Seuls la ligne sélectionnée et les cadres de détail utilisent la surface `--surface`. |

Un disque s'identifie **toujours** par modèle, numéro de série et WWN / EUI-64, **jamais** par un chemin `/dev/...`.

---

## 2. Tokens

Thème sombre « Schiste », le seul livré pour l'instant. À déclarer en variables CSS ou dans l'équivalent du framework.

### Couleurs de base

| Token | Valeur | Usage | Contraste |
|---|---|---|---|
| `--bg` | `#1B1F21` | fond de page | — |
| `--surface` | `#2C3337` | cadres de détail, ligne sélectionnée | — |
| `--fg` | `#ECEAE3` | texte principal, icônes neutres | 14:1 sur `--bg` |
| `--fg-2` | `#C9CABD` | texte secondaire | 10:1 sur `--bg`, 7.7:1 sur `--surface` |
| `--line` | `#4A5256` | séparateurs (en-tête, liste, crédits) | décoratif |
| `--ctrl` | `#959C9A` | contours de cadres et de contrôles | 5.9:1 |

### Couleurs d'état

| État | Texte, contour, icône | Pastille pleine | Texte de la pastille |
|---|---|---|---|
| Prêt (À lancer) | `--ready` `#6CC791` | `--ready-fill` `#2F7D4F` | `#FFFFFF` |
| Bloqué (À débloquer) | `--blocked` `#E8B04A` | `#E8B04A` | `#1B1F21` |
| Refus | `--refus` `#F0857C` | `--refus-fill` `#B3261E` | `#FFFFFF` |
| Protégé | `--protected` `#959C9A` | contour seulement, pas de remplissage | `--fg-2` |

Toutes ces paires dépassent 4.5:1, et les variantes claires dépassent 7:1 sur `--bg`.

**Règle de couleur.**
- Les **boutons** ne sont jamais remplis de couleur : ils ont un contour de 2 px et une icône ou un texte de la couleur de l'état.
- Les **pastilles** d'état (onglet Effacements, statut d'un certificat) sont **remplies**. Le détail d'un disque n'en a pas : la liste montre déjà l'état, et une pastille colorée au-dessus des méthodes ressemblait à un bouton (retirée le 2026-10-28).
- Aucune information ne repose sur la couleur seule : il y a toujours une icône ou un libellé.

### Typographie

| Rôle | Police | Taille / graisse |
|---|---|---|
| Marque « Disphas » (en-tête uniquement) | Klaudia Bold (`polices/klaudia/`) | 26 px |
| Interface | Atkinson Hyperlegible Next (Google Fonts) | corps 17 px, interligne 1.55 |
| Données techniques : s/n, WWN, EUI, ID de méthode, codes, compteurs | JetBrains Mono | 12 à 14 px |

- Klaudia **ne sert qu'au mot « Disphas »**, jamais ailleurs dans l'interface.
- Titres : H1 de la liste 25 px / 700, nom du disque dans le détail 40 px / 700 (32 px en tablette, 28 px en mobile).
- Titres de groupe : 15 px / 700, en minuscules normales (pas de capitales espacées).

### Divers

- Rayons : 4 px pour les boutons, 6 px pour les lignes et avertissements, 8 px pour les cadres.
- Cible tactile minimale : **44 × 44 px**.
- Focus clavier : `outline: 3px solid var(--fg); outline-offset: 2px`, visible sur tous les éléments interactifs.

---

## 3. Structure et composants

### 3.1 En-tête (partagé entre les onglets)

- **À gauche :**
  - « Disphas » en Klaudia ;
  - les onglets **Disques** / Effacements / Historique / Vérifier.
  - L'onglet actif est en gras, avec un soulignement de 2 px de couleur `--fg` et `aria-current="page"`. Les autres onglets sont en `--fg-2`.
- **À droite :**
  - l'état du poste en mono 14 px `--fg-2` : « hors ligne · assistant connecté · licence active · règles 1.0.0 » (valeurs dynamiques) ;
  - le bouton **Paramètres** : roue seule, sans contour ni texte, 44 px, avec `aria-label` et `title` « Paramètres ».
- Pas de bouton de langue ni de thème dans l'en-tête : ces réglages vont dans Paramètres.

### 3.2 Colonne liste

1. **En-tête de liste**
   - À gauche :
     - « Disques · {n} » (H1) ;
     - dessous, « {en cours} en cours · {max} simultanés permis » en mono 14 px `--fg-2` ;
     - dessous, « Hors lot : chaque effacement reçoit son propre certificat. » en 13 px, italique, `--fg-2`.
   - À droite, deux boutons-icônes sans contour : **+** (« Nouveau lot ») puis **↻** (« Actualiser »).
   - Le bouton **+** est le seul point d'entrée pour créer un lot.
2. **Groupes**, dans cet ordre, en masquant un groupe vide :
   - À lancer (`ready`) ;
   - À débloquer (`blocked`) ;
   - Refusés (`refus`) ;
   - Protégés (`protected`).
   - Titre du groupe : point de 8 px + « Libellé · {n} », dans la couleur de l'état.
3. **Ligne de disque** : hauteur minimale 64 px. Elle contient **deux boutons frères**, jamais imbriqués.
   - **Bouton d'action** : 44 × 44 px, contour de 2 px et icône dans la couleur de l'état, fond transparent, centré verticalement sur le texte.

     | État | Icône | Action | `aria-label` |
     |---|---|---|---|
     | Prêt | ▶ flèche pleine | lancer l'effacement avec la méthode retenue | « Effacer {modèle} · {Purge/Clear} » |
     | Bloqué | cadenas | Inspecter à nouveau (B1) / Déverrouiller… (B2) | « {action} {modèle} » |
     | Refus | ✕ croix | Forcer l'effacement si possible (N1), sinon afficher le détail | « Forcer l'effacement de {modèle} » ou « Voir pourquoi {modèle} est refusé » |
     | Protégé | bouclier | afficher le détail | « Voir pourquoi {modèle} est protégé » |

     Le bouton d'action Protégé a un contour de 1 px en `--ctrl` et une icône en `--fg-2`.
   - **Bouton de sélection** : occupe le reste de la ligne et porte `aria-pressed`.
     - Ligne 1 : le modèle en 16 px (tronqué par une ellipse si trop long) et, à droite, le type en mono 14 px `--fg-2`.
     - Ligne 2 en 14 px `--fg-2` :
       - pour un disque prêt, « {Purge/Clear} · {nom de la méthode retenue} » ;
       - sinon, le motif avec son code, par exemple « Sécurité ATA gelée (B1) ».
   - **Ligne sélectionnée** : fond `--surface` et contour 1 px `--ctrl`. **Pas** de barre colorée à gauche.
4. **Crédits**, en pied de colonne, collés en bas avec un filet `--line` au-dessus : « Crédits » à gauche (15 px / 600, `--fg-2`) et le solde à droite en mono 14 px. Le contenu exact est à définir ; la maquette affiche « [N] effacements restants ».

### 3.3 Colonne détail

**En-tête du disque**
- « {type} · {capacité} » en mono 14 px `--fg-2` ;
- le **modèle** (H2, 40 px / 700) ;
- « s/n {sn} · {WWN|EUI-64} {valeur} » en mono 14 px `--fg-2`.
- Pas de pastille d'état : l'état se lit dans la liste, et la pastille était prise pour un bouton.

**Cases et boutons radio** (révisé le 2026-10-28)
- Tous dessinés à la main (`appearance: none`) : sous WebKitGTK, le moteur de l'app sous Linux, les contrôles natifs suivent le thème GTK (case gris foncé, coche décentrée).
- Une seule famille : 20 × 20 px, contour 2 px `--ctrl`, `--fg-2` au survol. Case : rayon 4 px. Radio : rond.
- Choisi : couleur os `--fg`. Case : fond `--fg` et coche `--bg` centrée (SVG). Radio : contour `--fg` et point `--fg` de 8 px centré. Le vert reste réservé à l'état « prêt », il ne marque pas une sélection.
- « Tout sélectionner » : dans la colonne des cases des disques, juste au-dessus d'elles ; un tiret quand une partie seulement est cochée.
- Le `label` qui entoure chaque contrôle est la zone de clic (au moins 24 × 24 px, WCAG 2.2 critère 2.5.8).
- Focus clavier : le contour de 3 px suit la forme du contrôle.

**Disque prêt**
1. **Bloc « Décision de Disphas »**, **sans cadre** (révisé le 2026-10-27) : titre 15 px / 700 `--fg-2`, comme un titre de groupe de la liste ; la première méthode est à la hauteur du premier disque.
2. **Une ligne par méthode** : un `label` cliquable de 64 px minimum, comme une ligne de disque, qui contient :
   - un **bouton radio** de 20 px, dessiné à la main (`appearance: none`), voir « Cases et boutons radio » ci-dessous ;
   - la **catégorie** dans une étiquette de 52 px en mono 13 px : `Purge` remplie (`--fg` sur `--bg`), `Clear` en contour 1 px `--ctrl` ;
   - le **nom de la méthode** en 17 px ;
     - la méthode cochée est **en gras** ; la ligne cochée est encadrée comme le disque sélectionné (contour 1 px `--ctrl`, fond `--surface`) ;
     - la première méthode porte « · **recommandée** », ce dernier mot en vert `--ready` et en gras ;
   - sous le nom, l'**identifiant technique** en mono 14 px `--fg-2` (par exemple `AtaSanitizeOverwrite`).
3. **Avertissements**, s'il y en a : un cadre par note, contour 1 px `--ctrl`, icône triangle, le texte, puis le code (D4, R6…) en mono `--fg-2`. Ils restent neutres pour l'instant, sans couleur.
4. **Ligne d'action** :
   - le bouton **« ▶ Effacer »** : seulement le mot « Effacer », sans la catégorie ; contour 2 px et texte `--ready`, fond transparent, 52 px de haut, 18 px / 700 ;
   - à sa droite, en 15 px `--fg-2` : « {nom de la méthode} · certificat avec réserves | sans réserve ». C'est « avec réserves » dès qu'il y a au moins un avertissement.

**Disque bloqué, refusé ou protégé**
1. **Cadre d'explication** : même style que le cadre de décision (fond `--surface`, contour `--ctrl`, sans liseré ni fond teinté).
   - Le message principal est en 21 px.
   - Les motifs suivent en liste à puces `--fg-2`, chacun suivi de son code mono entre parenthèses.
   - Pour un forçage possible, ajouter : « Forçage possible : réécriture limitée à Clear, certificat NON CONFORME. »
2. **Bouton d'action**, s'il y en a un : contour 2 px et texte dans la couleur de l'état.
   - Ambre : « Inspecter à nouveau », « Déverrouiller… ».
   - Rouge : « Forcer l'effacement ».
   - Un disque protégé ou en lecture seule (N3) n'a pas de bouton.

---

## 4. Responsive

| Largeur | Comportement |
|---|---|
| > 1100 px | Grille à deux colonnes : liste de `minmax(340px, 420px)` et détail de `1fr`. Le détail a un padding de 40 × 48 px et une largeur maximale de 960 px. |
| 761 à 1100 px | Liste de `minmax(300px, 360px)`, détail avec 32 px de padding, nom du disque en 32 px. L'état du poste peut passer à la ligne sous les onglets. |
| ≤ 760 px | **Une seule vue à la fois.** La liste occupe tout l'écran ; toucher un disque, ou son bouton d'action, ouvre le détail. Un bouton « ‹ Disques » ramène à la liste. |

Détails du mobile (≤ 760 px) :
- **En-tête** sur deux rangées :
  - « Disphas » à gauche et la roue Paramètres à droite ;
  - puis les onglets sur toute la largeur, avec un défilement horizontal sans barre visible.
- L'état du poste quitte l'en-tête et s'affiche sous l'en-tête de la liste.
- Dans le détail, les boutons d'action prennent toute la largeur.
- Changer de vue renvoie en haut de page et met le focus sur le titre du disque.
- Revenir à la liste remet le focus sur la ligne du disque.
- Aucun défilement horizontal de page à aucune largeur (vérifié à 1440, 900 et 390 px).

---

## 5. Accessibilité (obligatoire)

- Contrastes : texte ≥ 4.5:1 ; texte secondaire et couleurs d'état ≥ 7:1 sur `--bg`.
- Cibles tactiles ≥ 44 × 44 px : boutons-icônes, boutons d'action de ligne, lignes de méthode.
- Les boutons-icônes ont tous un `aria-label` et un `title`.
- Les SVG ont `aria-hidden="true"`.
- La liste est dans un `aside` étiqueté « Disques connectés ». Chaque groupe est une `section` avec un `h2`. La sélection est exposée par `aria-pressed` sur le bouton de sélection.
- Les méthodes sont de vrais boutons radio natifs, groupés par `name`, dans un `fieldset` avec une `legend`. La navigation aux flèches est native.
- Le détail est en `aria-live="polite"` pour annoncer le changement de disque.
- Le focus est toujours visible (contour de 3 px) et ne doit jamais être supprimé.
- Jamais de couleur seule : chaque état a aussi une icône et un libellé.
- Respecter `prefers-reduced-motion`.

---

## 6. Données attendues

L'écran se nourrit de ce que l'application expose déjà. Les noms ci-dessous sont indicatifs, à faire correspondre au modèle existant.

```ts
type DriveState = 'ready' | 'blocked' | 'refus' | 'protected';

interface Drive {
  serial: string;          // s/n, identifiant principal
  model: string;
  bus: string;             // "NVMe", "SATA HDD", "SATA SSD", "Inconnu"
  capacity: string;        // déjà formatée : "500,1 Go", "1 To"
  idKind?: 'WWN' | 'EUI-64';
  idValue?: string;
  state: DriveState;
  methods?: { category: 'Purge' | 'Clear'; label: string; id: string }[]; // index 0 = recommandée
  notes?: { code: string; text: string }[];   // D4, R6… → certificat "avec réserves"
  reasons?: { code: string; text: string }[]; // B1, B2, N1, N3, P1, P2…
  message?: string;        // phrase principale pour les états non prêts
  action?: string;         // libellé du bouton d'action, si une action existe
  canForce?: boolean;      // N1 : forçage possible, limité à Clear, NON CONFORME
}
```

- Libellés des groupes : À lancer, À débloquer, Refusés, Protégés.
- Les données de démonstration de la maquette sont celles du mode démo de l'application.

---

## 7. À ne pas faire

- Pas de Klaudia ailleurs que sur le mot « Disphas ».
- Pas de boutons remplis de couleur : les boutons sont en contour, seules les pastilles sont remplies.
- Pas de liseré coloré en haut des cadres, et pas de barre colorée à gauche de la ligne sélectionnée.
- Pas de fond teinté (rouge ou ambre) dans les cadres d'explication.
- Pas de contour autour des boutons-icônes Actualiser, Nouveau lot et Paramètres.
- Pas de chemin `/dev/...` pour identifier un disque.
- Pas de petites capitales espacées pour les libellés : écrire en minuscules normales.
- Pas de boutons imbriqués : le bouton d'action et le bouton de sélection d'une ligne sont frères.

---

## 8. Points ouverts

- **Crédits** : unité et contenu exacts à définir (solde, échéance, lien de rachat). Garder l'emplacement prévu.
- **Avertissements D4 et R6** : neutres pour l'instant ; une teinte d'avertissement pourra venir plus tard.
- **Thème clair « Registre »** : non spécifié ici. Prévoir des tokens surchargeables.
- **Charte** : le vert sert désormais aussi à « prêt / recommandée », et l'ambre est ajouté pour « à débloquer ». La charte réservait jusqu'ici le vert à CONFORME : à reporter dans `DESIGN.md`.
