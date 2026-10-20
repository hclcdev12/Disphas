# disphas — Reprise du projet (passation entre agents)

État au **2026-10-20**. À lire **en premier** par tout nouvel agent, juste après `CLAUDE.md`. Ce document
dit où en est le projet, comment travailler avec le propriétaire et comment piloter le banc de test. Les
specs restent dans les documents listés par `CLAUDE.md` §1 ; ce fichier ne les remplace pas.

## 1. Où en est le projet

Le produit V1 est **complet et testé sur du vrai matériel** pour SATA (SSD et HDD) :

- Helper root, moteur de décision, protections P1–P5, 9 contrôles de `Sanitize`, signature Ed25519, PDF/A,
  `disphas-verify`, licences et révocation, lots et effacement parallèle, paquets `.deb`/`.rpm` (6 distributions
  testées en CI), application Tauri (Disques, Effacements, Historique, Vérifier, réglages).
- **Revue de sécurité terminée** (S1–S11, L1–L12) : voir [sécurité](securite.md) §5.
- **Banc de test (2026-10-06)** : effacements réels réussis et certificats vérifiés hors ligne :

| Disque | Méthode | Résultat |
|---|---|---|
| sdc Toshiba MK1655GSX | réécriture hôte (+ passe de zéros) | CONFORME, Clear, R7 |
| sdb Hitachi HTS723232A7A364 | ATA Security Erase renforcé (68 min) | CONFORME, **Purge**, R6 |
| sda SK hynix SC210 | ATA Security Erase renforcé (53 s) | CONFORME, Clear, R6 |
| sda + sdc en parallèle, en lot | Security Erase + réécriture | lot authentique, PDF de lot produit |
| sda, paquet de `472cec3`, lot sans groupe (2026-10-07) | ATA Security Erase renforcé (< 1 min) | CONFORME, Clear, R6 ; lot clos, PDF de lot authentique |

Décisions récentes prises sur le banc (toutes documentées dans les specs) :

- Racine *live* (PXE) suivie couche par couche : pas de disque système local si tout est en RAM/réseau
  ([moteur](moteur_decision.md) §1).
- Mot 217 ATA non déclaré → traité comme SSD, certifié avec le type `SATA` (« type non déclaré »)
  ([moteur](moteur_decision.md), [certificat](certificat.md) §2).
- Portée « tous les namespaces » signée (`san.nsa`) ([certificat](certificat.md) §2).
- Compte à rebours de l'estimation du disque pendant un Security Erase (aucun avancement firmware).
- **Certificats lus par le helper** (`ListReports`, `GetReport`), root seul, **plus de groupe `oblivio`**
  ([protocole](protocole_ipc.md) §10) — commit `472cec3`.

## 2. Points ouverts

**Renommage Oblivio → disphas (2026-10-13)** : code, site, paquets, chemins (`/var/lib/disphas`, `/usr/libexec/disphas`),
variables `DISPHAS_*`, action polkit `org.disphas.helper` et préfixes de format ([certificat](certificat.md) §1) renommés.
Gardent l'ancien nom : documents d'origine, logos de `assets/logo/` (et sa copie du site), domaines `oblivio.eu` et
`licence.oblivio.eu` (à trancher par le propriétaire), alias SSH `oblivio-bench`, dépôt GitHub `hclcdev12/Oblivio`,
groupe `oblivio` supprimé par le paquet, en-têtes `OBR1` (clé racine) et `OBK1` (clé du poste). Reste à faire :
~~logos disphas~~ (livrés le 2026-10-16, voir ci-dessous), recherche de marque, refaire le certificat d'émetteur (`DSI1:`) et
réémettre les licences (`DSL1:`) ; `setup.sh` relancé au démarrage du banc installe déjà la nouvelle règle polkit.

**Offres et nouvelle identité (2026-10-20) — la suite du projet**

- [Offres](offres.md) : niveaux d'offre et codes figés (`Essai`, `Passe`, `Pro`, `Atelier8`, `Atelier16`,
  `Atelier32`, `Parc`), droits par offre décidés par le helper (nombre de disques en parallèle, logo client), plus
  d'offre « à vie », ISO de démarrage avancée au lancement. Corrections des autres documents (§6) faites le
  2026-10-20. **Prochaine tâche : lot 1** (table des droits dans `disphas-core`), puis les lots 2, 3, 6 dans
  l'ordre du §8 ; chaque lot attend la validation du propriétaire.
- Identité disphas livrée par le graphiste : dossier de marque **hors dépôt** `~/Documents/graphism_Disphas/`
  (`README.md`, `DESIGN.md` = jetons de couleur, typographie, composants ; `IDENTITE.md` = nom, ton, certificat).
  Copiés dans le dépôt : logos (`assets/logo/disphas/`) et police Klaudia (`assets/fonts/klaudia/`), pas encore
  utilisés par le code. L'ancienne copie de travail `graphism_Disphas/` à la racine du dépôt a été supprimée le
  2026-10-20 (jamais suivie par git) : seul le dossier de `~/Documents` fait foi.
- Points ouverts de l'identité, à soumettre au propriétaire :
  - **wordmark du certificat** (état 0, encre, sans hermine) non livré ; nécessaire au lot 3 ;
  - **application et site** : aucun lot ne prévoit encore de passer à la nouvelle charte (couleurs Schiste/Registre,
    Klaudia, icônes de l'app) ; ils gardent la charte Oblivio ;
  - le certificat cite « IEEE 2883-2022 » dans ses référentiels, alors qu'O8 interdit « conforme IEEE 2883 » dans
    les supports commerciaux avant le lot 7 : à trancher ;
  - la charte écrit la commande `disphas verify`, le binaire s'appelle `disphas-verify` ;
  - les SVG contiennent des métadonnées de provenance (C2PA) à retirer avant de les embarquer, sur accord.

**À faire ensuite (banc)**

1. ~~Tester le paquet de `472cec3` sur le banc~~ → **fait le 2026-10-07**, tout est conforme (détail dans
   [banc de test](banc_de_test.md)).
2. NVMe : brancher un SSD NVMe via une **carte PCIe → M.2 passive** (le banc n'a pas de M.2 ; un pont USB
   RTL9210B ne laisse passer qu'Identify et Get Log Page, cf. smartmontools `scsinvme.cpp`). Puis Sanitize,
   Format, `nsa`, reprise après coupure. Le propriétaire doit déclarer le disque sacrifiable (CLAUDE.md §2).
   En attendant : **NVMe émulé** par QEMU sur le banc (2026-10-09, [banc de test](banc_de_test.md)) ; il a révélé
   qu'un NVMe sans Sanitize était refusé (SAY NO), corrigé. Reste à y faire : Format de bout en bout (licence à
   signer), S5 (voisin monté), et mettre les scripts de la VM dans le dépôt.
3. Reste de [banc de test](banc_de_test.md) : `Unlock` d'un disque verrouillé, disque *Frozen*, coupure de courant,
   Exit Failure Mode (B5), automount (S11), NVMe multi-namespaces (S5). Loop (S9) : validé le 2026-10-07.
   Suites d'interface de S9, corrigées le 2026-10-07 : nouveau code `DEVICE_IN_USE` (message clair au lieu de
   « Erreur interne du helper ») ; le libellé de P4 cite le périphérique loop. Vérifié sur le banc (paquet
   `0.1.0+build.12`) ; un disque devenu protégé entre verdict et ouverture répond `PROTECTED` (test du helper
   seulement : la course n'est pas reproductible à la main).
4. Persistance de `/var/lib/disphas` sur le banc (option (b) : NFS `no_root_squash`) : prérequis de tout test de
   coupure pendant un Security Erase.

**Décisions à soumettre au propriétaire**

5. ~~`dev.typ` liste fermée~~ → **décidé le 2026-10-07** : liste ouverte comme les codes, type inconnu affiché tel
   quel ([certificat](certificat.md) §2).
6. ~~Numéro de version distinct par paquet~~ → **décidé le 2026-10-07** : build hors tag en `0.1.0+build.<n° du run>`,
   tag `v*` = version du workspace (`packaging/README.md`, *Version*). Processus de release complet : plus tard.
7. ~~Après un effacement, la carte du disque revient à l'ancienne décision~~ → corrigé le 2026-10-07 : l'app
   réinspectait déjà, mais la nouvelle décision est la même ; la carte affiche désormais le dernier effacement de la
   session et son statut. Vérifié sur le banc (Security Erase de sda, CONFORME).
8. ~~Groupe `oblivio` résiduel (ancien nom du produit)~~ → **décidé le 2026-10-07** : supprimé par le paquet à l'installation ou à la mise
   à jour (groupe système seulement), vérifié par la CI *Package* et sur le banc (mise à jour `0.1.0` →
   `0.1.0+build.11` sans `--reinstall`, groupe de test supprimé, historique intact).

**Défauts d'interface constatés le 2026-10-07 (banc)**

9. ~~Historique : sélecteur de lot blanc sur blanc~~ → corrigé le 2026-10-07 (WebKitGTK dessinait les `<select>`
   avec le thème clair du système ; l'app les dessine elle-même, `color-scheme: dark`). Vérifié sur le banc ; le menu
   déroulant natif lui-même n'apparaît pas dans les captures `xwd`, non vérifié.
10. ~~Historique trié par identifiant (début) mais daté de la fin~~ → corrigé le 2026-10-07 : tri par la date affichée.
    Vérifié sur le banc.

**Licences en ligne et site (décidé le 2026-10-11)**

11. [Licences en ligne](licences_en_ligne.md) validées : clé boutique, licence v2, activation et renouvellement
    depuis l'application (jamais le helper), contrôle des postes, proxy d'entreprise (§5), serveur
    `licence.oblivio.eu`. Avancement (ordre du §10 de ce document-là) :
    - **Étape 1, QR** (2026-10-11) : imprimé à **5,5 cm** (`certificate.typ`), test du certificat le plus long.
      **Reste** : imprimer `largest-fr.pdf` (exemple `sample` de `disphas-report`, licence v2) et le scanner au téléphone.
    - **Étape 2, format et vérification v2** dans `disphas-attestation` (2026-10-11) : fait. La liste de révocation
      reste en `v: 1` (identifiants d'émetteur préfixés `ISS-`), simplification signalée au propriétaire.
    - **Étape 3, installation d'une licence v2 par le helper** (`disphas-license`, 2026-10-11) : fait, y compris
      le renouvellement (nouvelle licence installée par-dessus) et la révocation par l'émetteur.
    - **Étape 4, `disphas-issuer certify`** (2026-10-11) : fait (certificat d'émetteur `DSI1:…`, registre
      `issuers.csv`, révocation `ISS-…`). Testé en unitaire et sur les erreurs de la ligne de commande ; la cérémonie
      réelle (clé racine officielle, phrase secrète) reste au propriétaire.
    - **Étape 5, service de signature `disphas-signer`** (2026-10-11) : fait, testé en unitaire et de bout en bout
      (racine de développement). Restent, à l'hébergement : export du registre, chiffrement des clés au repos,
      installation réelle sous systemd.
    - **Suite** : étape 6, activation et renouvellement en ligne dans l'application (ou document d'architecture du
      site, dont dépend l'API publique §6.6).
    - Défauts préexistants constatés sur le certificat le plus long : le bloc Émetteur passe en page 2 (déjà à
      4,4 cm) ; un identifiant de lot de 64 caractères sans espace déborde de sa colonne.
12. Site : **Astro** dans `site/` (dépôt privé), production chez **Infomaniak** (préproduction possible sur le VPS
    Oracle gratuit du propriétaire), paiement **Stripe** ; facture électronique et TVA à valider par le comptable
    du propriétaire. Architecture : [site](site.md), validée le 2026-10-11 (site autonome dans `site/`). Étapes 1 (squelette) et 2
    (vitrine trilingue, contact) faites le 2026-10-11, ainsi que la **démo interactive** (helper simulé,
    faux PDF, audit avant publication : [site](site.md) §9). Étape 3 (comptes par lien de
    connexion, téléchargements, changement d'adresse, sessions ouvertes, suppression du compte, mot de passe facultatif
    en Argon2id, page de confirmation du lien) faite le 2026-10-11. Suite : Stripe, sièges et clés de licence (§9, étape 4).

**Plus tard** : dépôt public du vérificateur (depuis un **commit propre** : l'historique contient le n° de série
d'un vrai SSD), signature des paquets et release, vérification niveau audit, USB en V2, ISO live en V2.

**Côté propriétaire** : adresse security@, registre RGPD, validation juridique, achat IEEE 2883 puis ISO/IEC 27040,
CGV/EULA, relecture de l'allemand, site.

## 3. Travailler avec le propriétaire

- Il **ne code pas** : il supervise. Réponses en **français**, claires, avec le pourquoi des choix ; tableaux
  courts appréciés. Code, identifiants et commentaires en anglais.
- **On n'invente rien** : toute commande, option, chemin ou comportement est vérifié (code, config, doc
  officielle, exécution sans risque) avant d'être donné. Ce qui n'est pas vérifiable est dit comme tel.
- Il valide **étape par étape**. Une tâche finie → résumé + proposition de la suite, puis attendre son accord.
  Quand il dit « n'enchaîne pas », on s'arrête. Quand il dit « push seulement », on pousse sans rien lancer d'autre.
- **Push** uniquement sur son accord explicite (CLAUDE.md §5), après fmt + clippy + tests ; vérifier ensuite la CI et
  le dire. Le workflow *Package* se lance sur sa demande.
- Tout écart avec les specs ou décision qui les modifie : **le signaler et demander avant**. Mettre à jour `docs/`
  dans le même commit.
- Ne jamais manipuler ses mots de passe, la clé racine ni sa phrase secrète (même s'il les propose).

## 4. Le banc de test

Machine : Dell Precision T3500 (SATA ICH10 en AHCI, PCIe 2.0), **live PXE amnésique** (squashfs sur NFS
192.168.1.49, écriture en RAM), utilisateur `goupil`, 192.168.1.6, sur le même réseau que le poste de
développement. Disques déclarés sacrifiables : voir CLAUDE.md §2 (seuls ceux-là).

**Après chaque démarrage du banc** (tout est perdu à l'arrêt) :

1. Le propriétaire lance `sudo sh setup.sh` sur le banc (`tools/bench/setup.sh`, copié par SSH) : `sudo` sans mot
   de passe pour `goupil`, helper sans demande polkit, `xdotool`.
2. Copier et installer le paquet : `sudo apt-get install -y /tmp/disphas_<version>_amd64.deb` (avec le chemin,
   sinon `apt` cherche un paquet de ce nom). Depuis le 2026-10-07, chaque build a sa version (`0.1.0+build.<n>`) :
   `--reinstall` n'est plus nécessaire, `dpkg-query -W disphas` dit quel build est installé.
3. Lancer l'app sur l'écran du banc depuis SSH :
   `DISPLAY=:0 XAUTHORITY=$HOME/.Xauthority XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus setsid nohup /usr/bin/disphas-app > /tmp/disphas-app.log 2>&1 < /dev/null &`
4. Licence : l'app crée une demande `DSA1:…` (badge « Sans licence »). Le **propriétaire** la signe sur la machine
   qui détient la clé racine :
   `./target/release/disphas-issuer issue <chemin>/root.key --customer "Banc disphas" --offer Pro --days 30 'DSA1:…'`
   (le registre `issued-licenses.csv` est à côté de `root.key`). La licence `DSL1:…` se colle dans l'app.

**Voir et piloter** :

- Capture : `tools/bench/shot.sh <sortie.png> [échelle]` (xwd par SSH, aucune installation sur le banc), puis lire
  l'image. Les coordonnées `xdotool` sont celles de l'écran plein (multiplier par l'échelle).
- Clic et saisie : `ssh oblivio-bench 'export DISPLAY=:0 XAUTHORITY=$HOME/.Xauthority; xdotool mousemove X Y click 1'`,
  `xdotool type --delay 80 TEXTE`. Agrandir la fenêtre : `xdotool search --name "^disphas$"` puis `windowsize`.
- Suivre une tâche : relever par de **courtes** connexions répétées (`sudo -n ls /var/lib/disphas/jobs`,
  `/sys/block/sdX/stat`), jamais par une seule longue session SSH. Un Security Erase ne fait bouger aucun compteur :
  le fil du helper reste en état `D` jusqu'à la fin.
- Certificats : `sudo -n cat /var/lib/disphas/reports/<id>.cose` (et `.json`), vérifiés avec
  `./target/release/disphas-verify` **recompilé à jour** (un ancien binaire rejette les types récents).
- Test matériel bas niveau : `cargo test --release -p disphas-device --features hw-tests --test hw --no-run`, copier
  le binaire, puis `DISPHAS_TEST_DEVICE=sdX DISPHAS_TEST_SERIAL=<n° de série> sudo -n <binaire> --nocapture`
  (`DISPHAS_TEST_DESTRUCTIVE=I-ACCEPT-DATA-LOSS-ON-THE-TEST-DRIVE` et `--ignored` pour l'écriture).

## 5. Mettre en place un nouveau poste de développement

Prérequis vérifiés (documentation Tauri v2, *Prerequisites*, pour Fedora) :

```bash
sudo dnf check-update
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

- Rust : `rust-toolchain.toml` fixe le canal stable avec rustfmt et clippy (rustup l'applique seul).
- Node.js **24** (`.node-version`), par exemple avec fnm ; puis `npm ci` dans `app/`.
- `python3` (outils `tools/`), `pdftotext` pour relire un PDF en texte (paquet poppler).
- Clés **à recréer sur le nouveau poste** (ne pas copier les anciennes ; révoquer celles du poste précédent) :
  - clé de déploiement GitHub (accès en écriture) avec l'alias SSH `github-disphas` (le dépôt pointe sur
    `git@github-oblivio:hclcdev12/Oblivio.git`) ;
  - jeton `gh` *fine-grained* sur le dépôt : Actions lecture/écriture, Contents lecture ;
  - clé du banc et alias `oblivio-bench` (`HostName 192.168.1.6`, `User goupil`), installée avec `ssh-copy-id`.
- **Clé racine** (`root.key`) et `issued-licenses.csv` : jamais dans le dépôt ; ne les placer sur un poste que si les
  licences y sont signées, transférés par le propriétaire lui-même.
- Constaté sur Fedora 44 (2026-10-07) : `wget` est fourni par `wget2-wget` ; le poste n'a besoin d'aucun droit root
  hors installation des dépendances. Au premier SSH vers le banc, comparer l'empreinte affichée à celle lue sur
  l'écran du banc (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`). Le banc étant amnésique, la clé installée par
  `ssh-copy-id` est sans doute perdue à chaque redémarrage (non vérifié) : piste, l'intégrer à l'image PXE avec `setup.sh`.
- Les agents ne saisissent **aucun** mot de passe (règle de sécurité de Claude, indépendante de ce dépôt) : tout accès
  passe par clé SSH ou `sudo` sans mot de passe préparé par le propriétaire.
- Vérifier le poste : `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, puis dans
  `app/` `npm run lint` et `npm run build`. Démonstration sans matériel : `npm run demo` dans `app/` (port 1420 libre).
