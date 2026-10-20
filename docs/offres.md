# disphas — Offres commerciales et droits par offre

> **Statut : décidé en partie le 2026-10-20, à implémenter.** Rédigé à partir de l'audit de l'offre
> (2026-10-19/20) et des décisions du propriétaire. Ce document dit **ce que le produit doit permettre** pour
> vendre l'offre ; il ne fixe pas les prix, qui restent une **configuration du site** ([site](site.md) §5).
>
> **Place dans la documentation** : `CLAUDE.md` §1, point 6, après `licences_en_ligne.md`. En cas de contradiction, le [cadre](cadre_projet.md) prime ; ce document
> **remplace** la table des durées par offre de [licences en ligne](licences_en_ligne.md) §4 (voir §6).
>
> **Règles de travail inchangées** : `CLAUDE.md` (sécurité §2, règles invariantes §3, conventions §5).
> Chaque lot (§8) se termine par un résumé au propriétaire et attend sa validation. Les questions Q1 à Q5
> sont tranchées (§9) ; toute nouvelle décision qui modifie ce document lui est soumise **avant** d'écrire le code.

## 1. Décisions du propriétaire

| # | Décision | Date | Conséquence pour le code |
|---|---|---|---|
| O1 | **Cible prioritaire** : ateliers de reconditionnement et d'ITAD, en commençant par les petits et les associations | 2026-10-19 | Les fonctions « atelier » (parallèle, lots, logo) priment |
| O2 | Les ateliers effacent **disques démontés et machines entières** : l'**ISO de démarrage passe avant le lancement** | 2026-10-19 | L'ISO entre au lot de lancement (§8, lot 5) ; elle était en V2 ([cadre](cadre_projet.md) §1) |
| O3 | **« Atelier » est un niveau d'offre**, pas une durée de licence | 2026-10-19 | Corriger [licences en ligne](licences_en_ligne.md) §4 (voir §6) |
| O4 | **Pas d'offre « à vie »** : abonnement mensuel ou annuel uniquement | 2026-10-19 | Plus de `--perpetual` dans les offres vendues ; `disphas-issuer issue --perpetual` reste réservé aux cas manuels (D7) |
| O5 | **Effacements illimités** dans toutes les offres. Seul le **nombre de disques effacés en parallèle** dépend de l'offre | 2026-10-20 | Aucun compteur d'effacements, aucune télémétrie (règle invariante 8 inchangée) |
| O6 | **Prix fixes** : le prix affiché ne change pas quand l'éditeur doit facturer la TVA | 2026-10-19 | Site et Stripe : prix « TVA comprise » une fois assujetti ; mention « TVA non applicable, art. 293 B du CGI » d'ici là (§7) |
| O7 | **Aucune certification** (CSPN, Critères Communs…) avant que l'activité paie un SMIC | 2026-10-20 | Rien à prévoir dans le code |
| O8 | **IEEE 2883-2022** : achat du texte juste avant les ateliers pilotes, puis analyse d'écarts | 2026-10-20 | Lot 7 (§8) ; d'ici là, aucune mention « conforme IEEE 2883 » sur le site ni dans les supports commerciaux |
| O9 | Seul revenu annexe retenu : le **dossier de conformité**, offert aux clients Atelier | 2026-10-20 | Document à produire (§8, lot 8), pas de code |
| O10 | **Windows**, et l'**USB par l'ISO**, sont dans le périmètre **à terme** | 2026-10-20 | Hors lancement ; ne rien faire qui les empêche |

## 2. Grille des offres (piste retenue, prix non figés)

La grille ci-dessous est **retenue comme piste** le 2026-10-20 : les **niveaux** et leurs **droits** sont la base
du travail ; les **prix** restent à confirmer par les entretiens clients et vivent dans la configuration du site.

| Offre (code `off`) | Pour qui | Disques en parallèle | Logo du client | Durée de licence | Prix indicatif (TTC si TVA) |
|---|---|---|---|---|---|
| *(aucune licence)* — Diagnostic | Découverte | 0 : inspection et décision seulement | — | — | Gratuit |
| `Essai` | Essai avant achat | 8 | Oui | 30 jours, non renouvelée | Gratuit |
| `Passe` | Projet ponctuel | 8 | Oui | 30 jours, non renouvelée | 290 € une fois |
| `Pro` | Indépendant, ESN | 2 | Non | Abonnement | 300 €/an ou 30 €/mois |
| `Atelier8` | Petit atelier, association | 8 | Oui | Abonnement | 990 €/an ou 99 €/mois |
| `Atelier16` | Atelier moyen | 16 | Oui | Abonnement | 1 500 €/an ou 149 €/mois |
| `Atelier32` | Gros atelier, ITAD | 32 | Oui | Abonnement | 2 400 €/an ou 239 €/mois |
| `Parc` | Collectivité, multi-sites | Illimité | Oui | Abonnement ou licence v1 (D7) | Sur devis |

- Les codes d'offre sont **figés** (décidé le 2026-10-20, §9, Q5) : ne plus les changer, ils sont signés
  dans les licences et affichés sur les certificats.
- **Remises** (pionniers −50 % la 1re année, associations −25 %, engagement 2 ans −15 %) : **coupons Stripe**,
  sans effet sur la licence ni sur le code d'offre.
- **Poste de secours** : un siège de plus, sans frais, pour les offres Atelier. Côté licence, c'est un siège ordinaire.

## 3. État actuel du code (constaté le 2026-10-20)

| Besoin | État | Référence |
|---|---|---|
| Effacements illimités | ✅ Aucun compteur | — |
| Licence liée au poste, datée, renouvelable ; licence v2 signée par la clé boutique | ✅ | `crates/disphas-license/`, `tools/disphas-signer/` |
| Licence de 30 jours | ✅ `disphas-issuer issue … --days 30` | `tools/disphas-issuer/src/main.rs` |
| Offre écrite dans la licence (`off`) et imprimée sur le certificat | ✅ Texte libre, **purement informatif** | `public/disphas-attestation/src/model.rs:378`, `crates/disphas-report/src/lib.rs:415` |
| Effacement en parallèle | ✅ **Sans aucune limite** : seule la règle « une tâche par disque » s'applique | `crates/disphas-helper/src/session.rs:461` (contrôle 9) |
| Logo du client sur le certificat | ✅ **Ouvert à tous**, réglé côté application | `app/src-tauri/src/settings.rs`, `crates/disphas-report/src/lib.rs:96` |
| Droits par offre | ❌ Absents | — |
| Activation et renouvellement en ligne | ❌ Spécifiés, non codés (étape 6) | [licences en ligne](licences_en_ligne.md) §10 |
| Stripe, sièges, clés de licence | ❌ Spécifiés, non codés (étape 4 du site) | [site](site.md) §9 |
| Domaine du serveur de licences | ⚠️ `licence.oblivio.eu`, ancien nom | [licences en ligne](licences_en_ligne.md) §8 |
| Logo Disphas du certificat | ⚠️ Encore l'ancien wordmark Oblivio | `crates/disphas-report/src/lib.rs:36-38` (`oblivio-wordmark-light.svg`, `oblivio-stamp.svg`) |
| ISO de démarrage | ❌ Absente | [cadre](cadre_projet.md) §1 |

## 4. Spécification : droits par offre

### 4.1 Principe

- Un **droit** est une capacité du produit qui dépend de l'offre de la licence installée. V1 : deux droits.

| Droit | Type | Signification |
|---|---|---|
| `max_parallel` | entier ≥ 1, ou illimité | Nombre maximal d'effacements **en cours en même temps** sur le poste |
| `customer_logo` | booléen | Le logo du client remplace le wordmark disphas sur les certificats |

- **Les droits sont décidés et appliqués par le helper** (règle invariante 2 : l'UI ne décide rien).
- **Pas de nouvelle donnée réseau** : les droits se déduisent de l'offre de la licence déjà installée.

### 4.2 Où vit la table des droits

Deux options étudiées ; **décidé le 2026-10-20 : option A** (§9, Q1).

| Option | Description | Avantage | Inconvénient |
|---|---|---|---|
| **A — Table dans le helper** (recommandée pour le lancement) | Table versionnée `offre → droits`, fonction pure dans une crate sans I/O (proposition : `disphas-core`, module `entitlements`) | Aucun changement de format de licence ni du dépôt public | Une **nouvelle offre** demande une mise à jour du logiciel |
| B — Droits signés dans la licence | Champ supplémentaire de la licence v2 (ex. `par`), signé par la clé boutique, borné par le certificat d'émetteur | Nouvelles offres sans mise à jour | Change le format public (`disphas-attestation`), le vérificateur, et la taille du QR ([licences en ligne](licences_en_ligne.md) §3) |

B pourra venir plus tard sans casser A (un champ absent = table A) ; ce serait une nouvelle décision du propriétaire.

### 4.3 Règles de la table (option A)

1. Table **explicite**, versionnée comme le `RuleSet` (règle invariante 7 : aucun seuil en dur dispersé dans le code).
2. Valeurs de départ : celles du §2 (`Pro` 2, `Essai`/`Passe`/`Atelier8` 8, `Atelier16` 16, `Atelier32` 32,
   `Parc` illimité ; `customer_logo` faux pour `Pro`, vrai sinon).
3. **Offre inconnue** (licences v1 manuelles, offres futures) : **droits de `Pro`**, jamais un refus de signer
   (les licences v1 restent acceptées, D7). Décidé le 2026-10-20 (§9, Q2).
   `Dev` (racine de développement, builds de debug uniquement) : illimité.
4. Comparaison de l'offre **exacte** (casse comprise), sans interprétation du texte.

### 4.4 Limite de parallélisme (helper)

- Nouveau contrôle dans `Session::sanitize` (`crates/disphas-helper/src/session.rs`), **après** le contrôle 8
  (licence) et **avant** le démarrage de la tâche : si le nombre d'effacements en cours est déjà égal à
  `max_parallel`, refuser.
- Compte : les tâches d'effacement **en cours** de la session, y compris une tâche reprise après plantage.
  Une tâche terminée (succès, échec, REFUS) libère sa place.
- Refus : **nouveau code d'erreur** (proposition : `ParallelLimit`) dans `ErrorCode`
  (`crates/disphas-protocol/src/response.rs`), plutôt que `Busy`, qui signifie « ce disque est occupé ».
  Message : limite atteinte, nombre autorisé par l'offre.
- **Aucune tâche démarrée, aucun verdict consommé** en cas de refus : l'opérateur relance quand une place se libère.
- Le contrôle ne doit **jamais** être contournable par l'UI (pas d'option, pas de paramètre de commande).

### 4.5 Logo du client

- Aujourd'hui, le PDF est rendu par l'application (`disphas-report`, jamais dans le helper) : l'application
  n'est pas une frontière de confiance. **Décidé le 2026-10-20** (§9, Q3) :
  - **Retenu** : l'application lit `customer_logo` dans l'état de la licence (§4.6), masque le réglage et
    rend le wordmark disphas si le droit est absent. Contournable par quelqu'un qui modifie l'application :
    **limite assumée**, le logo étant cosmétique (il ne change rien à la preuve signée). À inscrire dans
    [sécurité](securite.md) §4.
  - Écarté : signer l'empreinte du logo dans l'attestation et la contrôler à la vérification (coût élevé,
    format public modifié, pour un enjeu cosmétique).
- Un logo déjà enregistré n'est **pas supprimé** quand le droit disparaît : il est simplement ignoré.

### 4.6 Protocole UI ↔ helper

- Ajouter les droits à `LicenseStatusResult` (`crates/disphas-protocol/src/response.rs:302`), par exemple
  `entitlements: { max_parallel, customer_logo }` ; régénérer les types TypeScript (`ts-rs`, `app/src/bindings`).
- Changement de format du protocole : appliquer la règle de version de [protocole](protocole_ipc.md)
  (`PROTOCOL_VERSION`, `crates/disphas-protocol/src/lib.rs:25`) et mettre ce document à jour dans le même commit.
- UI : afficher l'offre et ses droits dans la fenêtre Licence ; dans le tableau des disques, afficher
  « N effacements en cours sur M autorisés » ; message clair sur `ParallelLimit`.

### 4.7 Tests attendus

- Unitaires sur la table (chaque offre, offre inconnue, `Dev`).
- Session (`crates/disphas-helper/src/session.rs`, tests existants à étendre) : `Pro` refuse le 3e effacement
  simultané, accepte après la fin d'un des deux ; `Parc` accepte au-delà de 32 ; une tâche reprise compte.
- Application : réglage du logo masqué sans le droit ; PDF rendu avec le wordmark disphas.
- **Uniquement sur fichiers image ou loop devices**, et sur le banc pour les disques déclarés sacrifiables
  (`CLAUDE.md` §2).

## 5. Offres `Essai` et `Passe`

- Licences v2 de **30 jours**, émises par le service de signature comme les autres ; **non renouvelées**.
- Ajouter `Essai` et `Passe` aux offres du prochain certificat d'émetteur (`disphas-issuer certify --offers …`).
  Le plafond `maxd` (90 jours) les couvre déjà.
- Le certificat d'effacement porte l'offre (« … (Essai) ») : c'est voulu, un certificat émis pendant l'essai
  est réel et le dit.
- Site : `Essai` = un par client (adresse e-mail et organisation), sans carte bancaire ; `Passe` = paiement unique.

## 6. Corrections de documents existants

**Faites le 2026-10-20** (étape 0, avant le lot 1), sauf la page Tarifs du site, inchangée comme prévu.

| Document | Correction |
|---|---|
| [licences en ligne](licences_en_ligne.md) §4, table « Durée des licences » | Remplacer « Mensuelle (Atelier) / Annuelle ou à vie (Pro) » par une règle **par rythme de paiement**, indépendante de l'offre : mensuel → fin de période + 7 jours ; annuel → fin de période + 7 jours, plafonnée à 90 jours ; `Essai`/`Passe` → 30 jours, sans renouvellement. Retirer « à vie » (O4). Mettre à jour la cérémonie type (`--offers` : liste du §2) |
| [cadre](cadre_projet.md) §1 | L'effacement en parallèle est **fait** : le retirer de « Reporté (V2+) ». L'**ISO** passe au lancement (O2). Windows et USB par l'ISO : « à terme » (O10) |
| [site](site.md) §5 | Formes d'offres : retirer « Achat unique (à vie) » ; ajouter `Essai` et `Passe` ; remises par coupons Stripe ; prix fixes TVA comprise (§7) |
| [conformite.md](conformite.md) §3 | Ajouter O7 (pas de certification avant SMIC) et O8 (achat d'IEEE 2883 avant les pilotes) |
| `docs/cahier_des_charges_…` §7.2 | Ancienne grille (89 €/an, 149 € à vie, 79 €/mois) : **supplantée** par ce document, ne pas s'en servir |
| `site/src/i18n/*` (page Tarifs) | Garder « Tarifs à venir » tant que les prix ne sont pas figés par le propriétaire |

## 7. Site et paiement (rappel des contraintes pour l'étape 4 du site)

- Produits Stripe : un par niveau (`Pro`, `Atelier8`, `Atelier16`, `Atelier32`), chacun en **mensuel** et
  **annuel** ; `Passe` en paiement unique ; `Parc` par facture.
- Changement de niveau en cours d'abonnement : au prorata (Stripe). Au changement, le site renouvelle les
  sièges avec la nouvelle offre. `Renew` ne change aujourd'hui que la date de fin
  ([licences en ligne](licences_en_ligne.md) §6.3). **Décidé le 2026-10-20** (§9, Q4) : `Renew` accepte
  aussi une nouvelle offre, que le service de signature contrôle contre le certificat d'émetteur ; même `id`,
  même poste, même client. Mettre à jour [licences en ligne](licences_en_ligne.md) §6.3 et §6.6, et les tests
  de `tools/disphas-signer`. Ce travail rejoint le lot 6.
- **TVA** : d'ici l'assujettissement, factures avec « TVA non applicable, art. 293 B du CGI » et prix sans
  mention TTC. Après : mêmes montants, affichés « TTC » avec le HT correspondant. Règles exactes à valider
  par le comptable du propriétaire.
- Les prix et les nombres de postes restent une **configuration**, jamais du code (décision du 2026-10-11).

## 8. Lots de travail (dans cet ordre)

Chaque lot : fmt, clippy et tests verts, docs mises à jour dans le même commit, résumé au propriétaire,
**attendre sa validation** avant le lot suivant.

| # | Lot | Fait quand | Dépend de |
|---|---|---|---|
| 1 | **Droits par offre** (§4.1 à §4.3) : table, codes d'offre figés, tests unitaires | Table testée | — |
| 2 | **Limite de parallélisme** (§4.4) et protocole (§4.6) | Tests de session verts ; essai sur le banc avec 3 disques sacrifiables sous licence `Pro` : le 3e est refusé, puis accepté après la fin d'un effacement | 1 |
| 3 | **Logo réservé** (§4.5) et **wordmark disphas** sur le certificat (remplacer les SVG Oblivio par le wordmark disphas, état 0, encre, d'après le dossier de marque
`~/Documents/graphism_Disphas/` ; ce fichier n'a pas été livré, voir `assets/logo/disphas/README.md`) | PDF vérifiés dans les deux cas ; limite assumée ajoutée à [sécurité](securite.md) §4 | 1 |
| 4 | **NVMe sur vrai disque** (carte PCIe → M.2 passive ; disque déclaré sacrifiable par le propriétaire) : Sanitize, Format, `nsa`, reprise après coupure | Tableau des résultats ajouté à [banc de test](banc_de_test.md) | Matériel acheté par le propriétaire |
| 5 | **ISO de démarrage** (Debian, USB et réseau) : document d'architecture d'abord, soumis au propriétaire | Document validé, puis ISO testée sur le banc | Décision du propriétaire sur la licence liée au support ou à l'opérateur ([cadre](cadre_projet.md) §6) |
| 6 | **Activation et renouvellement en ligne** (étape 6), **site : Stripe et sièges** (étape 4), `Renew` étendu à l'offre (§7), domaine renommé, offres `Essai`/`Passe` | Achat de bout en bout sur une instance de test Stripe, changement d'offre compris | 1 |
| 7 | **Analyse d'écarts IEEE 2883-2022** : points ⚠️ de [moteur](moteur_decision.md) §7, conditions du Crypto Erase, échantillonnage, champs du certificat | Écarts consignés dans [conformité](conformite.md) **sans reproduire le texte** ; corrections par `RuleSet` (`rules_version`) | Achat du texte par le propriétaire (O8) |
| 8 | **Dossier de conformité** (pour les clients Atelier) : fiche type pour le registre RGPD, procédure d'effacement type, correspondance NIST SP 800-88 Rev. 2 et guide ANSSI PA-097 | Documents relus par le propriétaire | 7 pour la partie IEEE |

## 9. Points tranchés par le propriétaire (2026-10-20)

| # | Question | Décision |
|---|---|---|
| Q1 | Table des droits dans le helper (A) ou droits signés dans la licence (B) ? | **A** : table dans le helper au lancement |
| Q2 | Droits d'une offre inconnue ? | **Ceux de `Pro`**, sans jamais refuser de signer ; `Dev` illimité |
| Q3 | Logo réservé à Atelier contrôlé par l'application seulement (contournable) ? | **Oui**, limite assumée, à documenter dans [sécurité](securite.md) §4 |
| Q4 | Changement d'offre d'un siège : nouvelle activation, ou `Renew` étendu à l'offre ? | **`Renew` étendu à l'offre**, borné par le certificat d'émetteur (le site ne peut choisir qu'une offre que l'émetteur autorise) |
| Q5 | Codes d'offre définitifs | **Ceux du §2** : `Essai`, `Passe`, `Pro`, `Atelier8`, `Atelier16`, `Atelier32`, `Parc` |

## 10. Hors périmètre (ne pas faire)

- Compter les effacements, plafonner un volume, envoyer quoi que ce soit au serveur en dehors de la licence (O5, règle 8).
- Offre « à vie » (O4).
- Services payants (installation, formation, matériel) : écartés pour l'instant (O9).
- Certification (O7).
- Windows, smartphones, SAS : à terme seulement (O10).
- Écrire « conforme IEEE 2883 » sur le site ou dans un support commercial avant le lot 7 (O8).
