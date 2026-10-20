# Logos disphas

Copie des fichiers livrés par le graphiste (dossier de marque `~/Documents/graphism_Disphas/`, copie du
2026-10-20). **Référence** : `IDENTITE.md` §3.4 et `DESIGN.md` de ce dossier de marque ; ne pas retoucher les
dessins ici, les reprendre depuis la source.

Les anciens fichiers `../oblivio-*.svg` restent en place tant que le code les référence
(`crates/disphas-report/src/lib.rs`, icônes de l'application, site) : leur remplacement est le **lot 3** de
`docs/offres.md`.

| Fichier | Usage | Règle |
|---|---|---|
| `disphas-logo-principal-{sombre,clair,encre}.svg` | Site, documents, en-têtes | Jamais sous 128 px de large : passer au petit logo |
| `disphas-petit-logo-{sombre,clair,encre}.svg` | Le D seul : favicon, onglet, menus | Même dessin de 16 px à 1 024 px |
| `disphas-icone-{sombre,clair}.svg` | Icône de l'application (D dans un carré arrondi, rayon 22 %, 1 024 px) | Source des icônes Tauri (`app/src-tauri/icons/`) |
| `disphas-grand-format-{sombre,clair,encre}.svg` | Écran de démarrage, couverture | Jamais sous 256 px |

- `sombre` : sur fond schiste `#1B1F21` ; `clair` : sur fond os `#ECEAE3` ou papier ; `encre` : impression une couleur (`#111110`).
- **Manque pour le certificat** : le wordmark seul, à l'état 0 (« donnée »), en encre, sans hermine
  (`IDENTITE.md` §6.4). Aucun fichier livré ne le contient : à demander au graphiste ou à composer en
  Klaudia Bold (`../../fonts/klaudia/`), puis à faire valider par le propriétaire.
- Les SVG contiennent des métadonnées de provenance (manifeste C2PA de l'outil de génération) : à retirer
  avant de les embarquer dans un binaire ou de les publier, après accord du propriétaire.
