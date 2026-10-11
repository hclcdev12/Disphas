# Oblivio — Site web, espace client et API de licences

> **Validé par le propriétaire le 2026-10-11**, avec les décisions du §0. Décisions antérieures (2026-10-11) :
> Astro, dossier `site/` du dépôt privé, production chez Infomaniak (préproduction possible sur le VPS Oracle
> gratuit), paiement Stripe, domaine `oblivio.eu`, API de licences sur `licence.oblivio.eu`
> ([licences en ligne](licences_en_ligne.md) §6.6). Versions relevées sur npm le 2026-10-11.

## 0. Décisions du propriétaire (2026-10-11)

- **Tout le site vit dans `site/` et ne dépend de rien d'autre du dépôt** : polices, logos, couleurs, images,
  textes y sont copiés. On le construit et on le déploie seul, sans l'application. Le service de signature
  (`tools/oblivio-signer`) reste à part et s'installe à côté sur le serveur.
- **Charte = mêmes couleurs, même esprit** que l'application ; pas de fichier ni de composant partagé.
- **Tarifs et nombre de postes non fixés** : le site ne les écrit pas en dur (configuration des offres, §5).
- **Langue** : celle du navigateur (`Accept-Language`) ; à défaut, **anglais**.
- **Clé de licence** (§6) et **rédaction des textes** (§10) : pas encore tranchés ; non bloquants avant l'étape 4.

## 1. Ce que fait le site

| Partie | Contenu |
|---|---|
| **Vitrine** (publique, FR/EN/DE) | Produit, fonctionnement (décision, Say NO, certificat), sécurité et conformité, tarifs, demande de devis, contact, pages légales |
| **Espace client** (connecté) | Abonnements, clés de licence, postes, téléchargement des paquets, factures |
| **Vente** | Stripe Checkout (page de paiement hébergée par Stripe) et portail client Stripe (moyen de paiement, factures, résiliation) |
| **API de licences** | `POST /v1/activate`, `POST /v1/renew` sur `licence.oblivio.eu`, appelées par l'application ([licences en ligne](licences_en_ligne.md) §6.6) |

Le site **ne voit jamais de numéro de carte** : le paiement se fait chez Stripe, le site reçoit les événements
(webhooks).

## 2. Pile technique

| Domaine | Choix | Version (npm, 2026-10-11) | Pourquoi |
|---|---|---|---|
| Cadre | **Astro**, pages statiques par défaut, pages et API à la demande (`export const prerender = false`) | 7.3 | Vitrine rapide sans JavaScript inutile ; espace client et API dans le même projet |
| Serveur | `@astrojs/node`, mode `standalone` (`node ./dist/server/entry.mjs`, `HOST`/`PORT`) | 11.1 | Un seul processus Node derrière Caddy |
| Style | Tailwind CSS 4 (`@tailwindcss/vite`) | 4.3 | **Le même que l'application** : mêmes jetons de couleur (§3) |
| Icônes | Lucide (`@lucide/astro`) | 1.55 | Les mêmes que l'application (`lucide-react`) |
| Multilingue | i18n intégré d'Astro : `locales: ["en", "fr", "de"]`, `defaultLocale: "en"`, `routing.prefixDefaultLocale: true` | — | URLs `/en/…`, `/fr/…`, `/de/…` ; `/` redirige selon `Accept-Language` (`Astro.preferredLocale`, page rendue à la demande), anglais à défaut |
| Base de données | PostgreSQL + Drizzle ORM (pilote `postgres`) | drizzle 0.45 | Comptes, abonnements, sièges, postes |
| Connexion | Better Auth, **lien de connexion par courriel** (plugin `magicLink`), adaptateur Drizzle `provider: "pg"` | 1.7 | Pas de mot de passe à stocker ; bibliothèque auto-hébergée, données chez nous |
| Paiement | `stripe` (Checkout, Billing, Customer Portal, Tax) | 23.0 | Décidé ; facture électronique et TVA à valider par le comptable |
| Courriels | `nodemailer` vers le SMTP de l'hébergeur | 10.1 | Liens de connexion, reçus, alertes. *Non vérifié* : SMTP inclus dans l'offre Infomaniak |
| Sécurité | `security.checkOrigin` (activé par défaut : refuse les POST d'une autre origine), `security.csp` (Astro ≥ 6) | — | Protection CSRF et politique de contenu sans code maison |

Aucun service tiers dans les pages : **ni analytique, ni police distante, ni script externe**. Stripe Checkout est une
redirection, pas un script intégré.

## 3. Charte graphique : les couleurs et l'esprit de l'application

Exigence du propriétaire (2026-10-11) : mêmes couleurs, même esprit que l'application
([charte](oblivio_charte_graphique.md)), le site restant **autonome** (§0).

- **Couleurs** recopiées dans `site/src/styles/global.css` (`@theme` de Tailwind 4), mêmes valeurs que la charte :
  fond `#090D16`, cartes `#111827`, bordures `#1F2937`, texte `#F9FAFB` / `#9CA3AF`, indigo de marque `#6366F1`,
  états vert `#10B981`, rouge `#EF4444`.
- **Polices** Inter et JetBrains Mono copiées dans `site/public/fonts/` avec leurs licences, servies par le site.
  Pas de Google Fonts.
- **Logos** copiés dans `site/public/` depuis `assets/logo/`, avec leurs règles (indigo seul, jamais de couleur
  d'état, zone de protection, taille minimale). Favicon : `oblivio-icon.svg`. Le **tampon** reste réservé aux
  certificats.
- **Esprit** : sombre, mat, industriel, sobre ; bordures fines, rayon 8 px au plus, pas d'ombre lourde ; indigo
  pour les actions principales ; vert et rouge seulement pour montrer des états (un certificat CONFORME, un refus
  Say NO), jamais pour décorer ; monospace pour les données techniques.
- **Normes** : NIST et IEEE cités dans le texte avec la mention de non-affiliation, jamais en sceau ni en logo.

## 4. Pages

**Publiques** (statiques, `/fr`, `/en`, `/de`) : accueil ; produit ; fonctionnement (décision unique, Say NO,
certificat signé, vérification hors ligne) ; sécurité et conformité ; tarifs ; télécharger (explique, puis renvoie
vers l'espace client) ; devis (formulaire) ; contact ; mentions légales ; CGV et contrat de licence ; confidentialité ;
politique de sécurité (signalement, `security@oblivio.eu`).

**Espace client** (à la demande, connecté) :

| Page | Contenu |
|---|---|
| Connexion | Adresse de courriel, lien valable 5 minutes (valeur par défaut de Better Auth) |
| Tableau de bord | Abonnements, échéances, postes utilisés / autorisés |
| Licences et postes | Clé de licence (§6), postes actifs (n° de licence, date d'activation, échéance), **Libérer**, **Ajouter un poste hors ligne** (coller `OBA1`), **Copier la licence renouvelée** (secours hors ligne) |
| Téléchargements | Paquets `.deb` / `.rpm`, empreintes SHA-256, notes de version, manuel |
| Facturation | Lien vers le portail Stripe (factures, moyen de paiement, résiliation) |
| Organisation | **Nom imprimé sur les certificats** (client de la licence) |

Le nom de l'organisation est inscrit dans chaque licence et repris tel quel aux renouvellements
([licences en ligne](licences_en_ligne.md) §6.3) : le changer exige de réactiver les postes. L'interface le dit.

## 5. Offres et droits

**Tarifs et nombre de postes : non fixés** (propriétaire, 2026-10-11). Les offres sont une **configuration** du
site (nom, prix Stripe, postes, rythme), pas du code. Formes prévues :

| Forme | Stripe | Licences |
|---|---|---|
| Gratuite (diagnostic) | — | Aucune ; compte requis pour télécharger |
| Abonnement (mensuel ou annuel) | Billing | Fin de période + 7 j, plafonnée à 90 j, renouvelée tant que l'abonnement court |
| Achat unique (« à vie ») | Paiement unique | 90 j, renouvelées gratuitement sans fin |
| Sur devis | Facture Stripe ou virement | Activation en ligne, ou licences v1 signées hors ligne |

Événements Stripe traités : `checkout.session.completed`, `customer.subscription.updated` / `deleted`,
`invoice.paid`, `invoice.payment_failed`. Un abonnement impayé ou résilié n'est plus renouvelé : ses licences
s'éteignent à leur échéance, rien n'est révoqué.

## 6. Données

| Table | Contenu |
|---|---|
| Comptes, sessions | Gérés par Better Auth |
| `organizations` | Nom imprimé sur les certificats, adresse de facturation, n° de TVA, client Stripe |
| `subscriptions` | Offre, statut, fin de période, identifiants Stripe |
| `license_keys` | **Empreinte** (SHA-256) de la clé de licence, jamais la clé : affichée une fois à sa création, **régénérable** (l'ancienne cesse aussitôt d'activer de nouveaux postes) |
| `seats` | Identifiant `OBL-…`, clé publique du poste, libellé, activé le, échéance courante, libéré le |
| `quotes` | Demandes de devis |
| `events` | Journal des actions sensibles (activation, libération, régénération de clé), sans donnée de disque |

**Rapprochement quotidien** registre du service de signature ↔ `seats` : une licence signée sans siège payé
déclenche une alerte ([licences en ligne](licences_en_ligne.md) §6.5).

## 7. Hébergement

```
Internet ─► Caddy (HTTPS automatique) ─┬─ oblivio.eu, www ─────────► Node (Astro) ─► PostgreSQL
                                        └─ licence.oblivio.eu/v1 ───┘      │
                                                                            └─ socket ─► oblivio-signer
```

- VPS Infomaniak, Debian, un seul serveur au départ ; préproduction identique sur le VPS Oracle.
- Unités systemd : site, `oblivio-signer` ([unité durcie](../tools/oblivio-signer/oblivio-signer.service)),
  PostgreSQL ; Caddy en frontal.
- Paquets téléchargeables stockés hors de la racine web, servis par le site après contrôle de la session.
- Sauvegardes quotidiennes chiffrées : base, registre des licences ; **pas** les clés boutique.
- Courriels : `contact@`, `security@` (exigé par le [CRA](conformite.md), C1), `noreply@`.

## 8. Sécurité et vie privée

- Better Auth conserve avec chaque session l'**adresse IP** et le **navigateur** (lutte contre les abus) :
  données personnelles à mentionner dans la politique de confidentialité, avec la durée de session (7 jours par
  défaut de Better Auth, constaté sur une session de test).
- Seul cookie : la session (strictement nécessaire). *À valider juridiquement* : pas de bandeau de consentement
  dans ce cas.
- CSP stricte (`security.csp`), aucune ressource externe ; en-têtes HSTS, `X-Content-Type-Options`,
  `Referrer-Policy` posés par Caddy.
- Liens de connexion à usage unique ; débit limité sur la connexion, le devis et l'API de licences.
- Webhooks Stripe vérifiés par signature ; actions idempotentes.
- Journaux du serveur courts ; politique de confidentialité ([conformité](conformite.md), R6).

## 9. Ordre de travail proposé

1. ~~**Squelette `site/`** autonome~~ → **fait le 2026-10-11** : Astro 7 (adaptateur Node), Tailwind 4, couleurs et
   polices de la charte copiées, logos, gabarit (en-tête, langues, pied de page avec mention de non-affiliation),
   `/` redirigé selon `Accept-Language` (anglais à défaut, `Vary: Accept-Language`), page d'accueil trilingue
   (textes tirés de l'identité de marque ; anglais et allemand à relire). Télémétrie d'Astro désactivée dans tous
   les scripts (`ASTRO_TELEMETRY_DISABLED=1`) ; les deux premiers builds, avant ce réglage, l'avaient active.
2. ~~**Vitrine trilingue**~~ → **faite le 2026-10-11** : accueil (aperçu de certificat fictif, cinq étapes, états,
   piliers, publics), produit, sécurité et conformité, tarifs **sans prix ni nombre de postes**, téléchargement
   (« bientôt disponible »), contact et devis (formulaire sans JavaScript : `POST /api/contact`, champ piège, débit
   limité à 5 messages par heure et par adresse, envoi par `SMTP_URL` / `CONTACT_TO` / `CONTACT_FROM`, simple
   journal en développement), pages légales en **emplacements non indexés**. Textes tirés du manuel et de
   l'identité de marque, limités à ce que fait le produit ; anglais et allemand à relire. **Avant la mise en ligne**
   : textes légaux (mentions légales obligatoires), adresse `security@`, configuration SMTP.
   **Démo interactive** (demandée par le propriétaire le 2026-10-11, option « vraie interface » retenue) : page
   `/[lang]/demo/`, qui ouvre `/demo/app/` dans un nouvel onglet. C'est l'interface de l'application construite en
   mode démo (`app`, `npm run build:demo`) sur un **helper simulé** (`app/src/demo/api.ts`) : disques fictifs de
   l'enregistrement du helper de démo, effacements rejoués en temps accéléré, cas d'erreur (SSD gelé puis REFUS
   N8 après une nouvelle tentative, disque verrouillé déverrouillé avec `oblivio` ou expiré B6, disque muet forcé
   NON CONFORME, NVMe en lecture seule SAY NO, disque système PROTÉGÉ), et **faux PDF** rendus par le vrai moteur
   avec la clé de test publique (`oblivio-report`, exemple `demo_certificates` : 60 fichiers, un par disque, méthode
   et langue). Assemblage : `tools/site-demo/build.sh`, qui **audite** le résultat avant de le copier dans
   `site/public/demo/app/` (`tools/site-demo/audit.sh` : aucune source map, aucun chemin local, aucun appel au vrai
   helper, aucune clé ni licence encodée, seulement des certificats `DEMO-*`). Le code exposé est celui de
   l'interface, minifié, sans source maps, le même que celui livré dans les paquets ; le helper, le moteur, la
   signature et les licences n'y sont jamais. 122 Ko de JavaScript compressé ; les PDF (5,5 Mo) ne sont chargés
   qu'à l'ouverture.
3. ~~**Comptes** (lien de connexion), téléchargements~~ → **faits le 2026-10-11** : Better Auth 1.7, plugin
   `magicLink` (lien valable 5 minutes, une seule fois ; compte créé à la première connexion), télémétrie désactivée
   explicitement ; base PostgreSQL (`DATABASE_URL`) ou PGlite en développement (`.data/pglite`, 0700) ; migrations SQL
   numérotées (`migrations/`, `npm run migrate`, sans `drizzle-kit` dont une dépendance avait une alerte de sécurité) ;
   connexion sans JavaScript (`POST /api/account/login`, même réponse que le compte existe ou non, 10 demandes par
   heure et par adresse IP, 3 par heure et par courriel) ; espace client `/[lang]/account/` (téléchargements,
   déconnexion ; licences à venir) ; téléchargements servis depuis `RELEASES_DIR` après contrôle de session, noms
   validés et listés, empreintes SHA-256 lues dans `SHA256SUMS`. L'adresse IP n'est lue dans `X-Forwarded-For` que
   si `TRUST_PROXY=1` (derrière Caddy) ; l'en-tête transmis à Better Auth est réécrit, un visiteur ne peut pas le
   forger. Hors développement, le site refuse de servir l'espace client sans `DATABASE_URL`, `BETTER_AUTH_SECRET`,
   `BETTER_AUTH_URL`. Testé : parcours complet, lien à usage unique, déconnexion, limites, requête intersite (403),
   téléchargement (identique à l'octet ; 401 sans session ; 404 pour tout nom non listé ou chemin détourné).
   **Configuration de production** : `DATABASE_URL`, `BETTER_AUTH_SECRET`, `BETTER_AUTH_URL`, `SMTP_URL`, `MAIL_FROM`,
   `CONTACT_TO`, `RELEASES_DIR`, `TRUST_PROXY=1`.
4. **Stripe** (Checkout, webhooks, portail), sièges, clés de licence, espace « Licences et postes ».
5. **API de licences** `/v1/activate`, `/v1/renew`, reliée au service de signature ; puis l'application (étape 6
   des [licences en ligne](licences_en_ligne.md)).
6. Déploiement : préproduction Oracle, puis production Infomaniak.

## 10. Points à trancher

1. ~~Nombre de postes de l'offre Atelier, tarifs~~ → **non fixés pour l'instant** (2026-10-11) ; configuration (§5).
2. ~~Langue par défaut~~ → **langue du navigateur, anglais à défaut** (2026-10-11).
3. **Clé de licence** (avant l'étape 4) : empreinte seule et régénération (recommandé : une base volée ne livre
   aucune clé), ou clé consultable à tout moment (plus pratique, mais stockée chiffrée sur le serveur).
4. **Textes** : je propose ceux de la vitrine à partir de l'[identité de marque](Oblivio%20—%20Identité%20&%20Narration%20de%20Marque.md),
   à relire par le propriétaire (l'allemand en particulier) ; les pages légales restent des emplacements non
   publiés tant qu'un juriste ne les a pas validées.
