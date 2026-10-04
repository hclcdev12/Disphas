# Oblivio — Conformité réglementaire et normative

État au **2026-10-04**. Document de travail technique, **pas un avis juridique** : les parties RGPD et
Cyber Resilience Act doivent être validées par un avocat ou un DPO avant la mise sur le marché.
Chaque référence a été vérifiée sur sa source officielle (§ 8) ; ce qui n'a pas pu l'être est marqué
**non vérifié**.

**Situation** : produit **pas encore commercialisé** ; éditeur en **micro-entreprise** ; logiciel 100 %
hors ligne (aucun appel réseau, aucune télémétrie — vérifié dans le code et la CSP de l'application).

## 1. RGPD (règlement (UE) 2016/679)

### 1.1 Rôles

| Acteur | Rôle | Pourquoi |
|---|---|---|
| Client utilisateur d'Oblivio | **Responsable de traitement** (ou sous-traitant de ses propres clients) | Il produit et conserve les certificats ; Oblivio ne transmet rien à l'éditeur |
| Éditeur (micro-entreprise) | **Non concerné** par les données des clients | Aucune donnée ne quitte le poste du client |
| Éditeur | **Responsable de traitement** de son registre de licences | `issued-licenses.csv` : nom du client (souvent une personne physique), offre, dates, clé du poste |

Oblivio **sert** la conformité des clients : effacement avant cession ou fin de location (art. 17,
28(3)(g) : suppression des données en fin de prestation ; art. 32 : sécurité), preuve signée (art. 5(2),
responsabilité). La CNIL (fiche n° 15 du guide sécurité, mise à jour 2026) recommande des logiciels
d'effacement **qualifiés ou certifiés** et renvoie aux certifications CSPN de l'ANSSI.

### 1.2 Données personnelles dans un certificat

| Champ | Donnée personnelle ? | Remarque |
|---|---|---|
| Opérateur (`op`) | **Oui** | Facultatif ; l'application ne le demande pas encore (`null`) |
| Organisation (`org`) | Si personne physique | Facultatif |
| N° de série, WWN/EUI-64 | **Possible** (art. 4(1), considérant 30) si rattachable à une personne (portable d'un salarié) | Indispensable à la preuve : finalité légitime, à documenter par le client |
| Justification de forçage | Possible (texte libre) | Avertir de ne pas y saisir de données personnelles |
| Client de la licence | Si personne physique | Intégré à chaque certificat (preuve d'origine) |

Un certificat signé est **immuable** par construction. Sa conservation relève de l'exception de
l'art. 17(3)(e) (constatation, exercice ou défense de droits en justice) ; c'est au client de fixer une
durée de conservation (art. 5(1)(e)).

### 1.3 Écarts et actions (privacy by design, art. 25)

| # | Constat | Action | Statut |
|---|---|---|---|
| R1 | Certificats en `0644` dans `/var/lib/oblivio/reports` : lisibles par **tous** les comptes du poste (art. 25(2), accessibilité) | Dossier `0750` et fichiers `0640`, groupe `oblivio` (créé par le paquet, qui restreint aussi les certificats d'une installation antérieure) ; opérateurs ajoutés par l'administrateur (`usermod -aG oblivio`) ; l'historique explique un refus d'accès | **Fait** (2026-10-04) |
| R2 | Aucune durée de conservation, aucune suppression possible depuis l'application | Suppression **manuelle** depuis l'historique (un certificat, un lot ou tout ce qui précède une date), confirmée, exécutée par le helper (`DeleteReports`) et consignée sans donnée personnelle (art. 5(2)). Pas de suppression automatique en V1 (risque de perdre des preuves) ; les PDF exportés restent à la charge de l'utilisateur | **Fait** (2026-10-04) |
| R3 | Champs opérateur/organisation à venir | Facultatifs, identifiant plutôt que nom accepté, mention d'information | À faire avec ces champs |
| R4 | Justification libre | Avertissement dans la boîte de forçage | À faire |
| R5 | Registre de licences de l'éditeur | Base légale (contrat), durée de conservation, mention dans les CGV ; **registre des traitements** probablement requis (art. 30(5) : traitement non occasionnel) ; registre gardé sur la machine hors ligne | À faire (éditeur) |
| R6 | Futur site web (VPS) | Politique de confidentialité, cookies | Hors périmètre produit |

## 2. Cyber Resilience Act (règlement (UE) 2024/2847)

- **Applicable** : Oblivio est un logiciel commercial, « produit comportant des éléments numériques » (art. 3(1)).
- **Classe** : l'effacement de données ne figure ni à l'annexe III (produits importants) ni à l'annexe IV
  (critiques) → **contrôle interne** (module A, art. 32(1)(a)), sans organisme notifié. *Non vérifié :*
  l'acte d'exécution détaillant les catégories de l'annexe III.
- **Calendrier** (art. 71) :
  - signalement des vulnérabilités activement exploitées et incidents graves (art. 14 : alerte 24 h,
    notification 72 h) **depuis le 11/09/2026**, y compris pour les produits mis sur le marché avant 2027 (art. 69(3)) ;
  - application complète (exigences essentielles, marquage CE) **le 11/12/2027**.
- **Micro-entreprise** : documentation technique simplifiée (art. 33(5)), pas d'amende pour le délai de
  24 h (art. 64(10)(a)), frais d'évaluation réduits (art. 32(6)), accompagnement des États membres (art. 33).
- **Exigence qui nous vise directement** : annexe I, partie I, (2)(m) — permettre à l'utilisateur de
  **supprimer de façon sûre et définitive toutes les données et tous les paramètres**.

| # | Action | Échéance | Statut |
|---|---|---|---|
| C1 | Procédure de signalement et contact sécurité (`SECURITY.md`), avant la première vente | Mise sur le marché | **En partie** : `SECURITY.md` rédigé (2026-10-04) ; à compléter : adresse dédiée (`security@<domaine>`, avec le site) et délai définitif (5 jours ouvrés indicatifs). Procédure interne ci-dessous |
| C2 | Désinstallation complète : clé du poste, licence, certificats, tâches, fichiers de reprise (annexe I (2)(m)) | 11/12/2027 | À faire |
| C3 | Revue des exigences essentielles de l'annexe I et documentation technique simplifiée | 11/12/2027 | À faire (à détailler sur le texte) |
| C4 | Marquage CE et déclaration UE de conformité (module A) | 11/12/2027 | À faire |

### 2.1 Procédure interne de signalement (art. 14) — dès la mise sur le marché

Si une vulnérabilité d'Oblivio est **activement exploitée**, ou en cas d'incident grave affectant
sa sécurité, l'éditeur notifie via la **plateforme unique de signalement** prévue par le CRA :
1. **alerte précoce sous 24 h** après en avoir eu connaissance (pour une micro-entreprise, pas
   d'amende en cas de dépassement de ce délai — art. 64(10)(a)) ;
2. **notification sous 72 h** ;
3. information des utilisateurs concernés et correctif.
*Non vérifié :* les modalités pratiques de la plateforme et l'autorité (CSIRT) compétente en
France — à préciser avec l'ANSSI avant la mise sur le marché.

## 3. Normes et référentiels techniques

| Référentiel | Statut vérifié | Position d'Oblivio |
|---|---|---|
| **NIST SP 800-88 Rev. 2** | Finale, septembre 2025, remplace la Rev. 1 | Conforme sur le fond (Clear / Purge / destruction ; validation ; certificat). Le NIST dit *destroy*, nous *Destruct* (terme d'IEEE 2883 et d'ISO/IEC 27040). Son annexe C propose un modèle de certificat : **comparaison champ à champ à faire**. Les techniques par support sont renvoyées à IEEE 2883 |
| **IEEE 2883-2022** « Standard for Sanitizing Storage » | En vigueur (publiée le 17/08/2022) | Référence technique annoncée. ⚠️ **Texte jamais lu** (payant) : l'implémentation s'appuie sur les spécifications NVMe et ACS et sur le NIST. **À acquérir** (§ 7) |
| **ANSSI PA-097** (reconditionnement, 30/06/2023) | Publié | **Aligné** : la réécriture ne suffit pas sur SSD ; Sanitize firmware et effacement cryptographique privilégiés |
| **CNIL**, guide sécurité, fiche n° 15 | Mise à jour 2026 | Oblivio répond à la recommandation (effacement sécurisé avant fin de vie) ; certification visée : § 3.2 |
| **BSI IT-Grundschutz CON.6** (Édition 2023) | Vérifié sur le PDF officiel | Voir § 3.1 |
| **ISO/IEC 27040:2024** « Storage security » | Existe ; contenu **non vérifié** (accès bloqué) | À acquérir (§ 7) |
| **ISO/IEC 27001:2022** | Contrôles d'annexe A **non vérifiés** | Certifie des organisations, pas des produits ; nos certificats servent de preuve aux audits des clients |
| **ISO/IEC 21964** (remplace DIN 66399, retirée) | 2018 | Destruction **physique** uniquement : concerne notre recommandation « Destruct » |

### 3.1 BSI CON.6 (décidé le 2026-10-04 : s'y aligner)

CON.6.A12 (exigence de base) impose : supports réinscriptibles **non chiffrés** → réécriture complète par
un **flux de valeurs aléatoires (PRNG)** ; supports **chiffrés** → suppression sécurisée de la clé.

| Méthode Oblivio | CON.6.A12 | Remarque |
|---|---|---|
| Écriture hôte | ✅ **Alignée** (depuis le 2026-10-04) | Flux ChaCha20 dérivé de la graine, validé bloc à bloc ([moteur](moteur_decision.md) §5). Reste certifiée Clear (règle invariante) |
| NVMe Sanitize Crypto Erase, ATA Crypto Scramble, NVMe Format (SES 2) | ✅ Point « clé » | Si le concept de chiffrement du client reconnaît le chiffrement intégré au disque |
| NVMe Sanitize / ATA Sanitize Block Erase, Format (SES 1), ATA Security Erase | ⚠️ Pas une réécriture aléatoire au sens littéral | Purge au sens NIST/IEEE, plus fort que la réécriture sur SSD (ANSSI PA-097). CON.6.A4 laisse l'institution évaluer ses procédés |
| ATA Sanitize Overwrite | ⚠️ Motif fixe (limite de la commande) | Purge NIST sur disque dur |

**Décision ouverte (propriétaire)** : pour un client qui exige la lettre de CON.6.A12, faut-il un
**profil « BSI »** qui ajoute une passe hôte pseudo-aléatoire après les méthodes ⚠️ (plus long, sans
changer la catégorie certifiée) ?

### 3.2 Certification visée : CSPN (ANSSI)

La CSPN (décret 2002-535) a une catégorie **« 4 – effacement de données »** ; la CNIL y renvoie. La seule
référence trouvée (Blancco Drive Eraser, ANSSI-CSPN-2021/04) est **non maintenue** au catalogue du
15/09/2026. *Non vérifié :* procédure, délais et coûts d'évaluation (laboratoire agréé) — à demander à l'ANSSI.

## 4. Export de cryptographie (règlement (UE) 2021/821)

Annexe I, catégorie 5 partie 2, 5A002.a, note technique 1 : la cryptographie servant uniquement à
l'authentification, la signature, l'intégrité ou la gestion des clés associées n'est pas de la
« cryptographie pour la confidentialité des données » → **non contrôlée**. Oblivio signe (Ed25519) et
utilise ChaCha20 **comme générateur pseudo-aléatoire** de réécriture, sans chiffrer de données. Le
chiffrement de la clé racine (Argon2id + ChaCha20-Poly1305) est dans l'outil de l'éditeur, **jamais livré**.
*Non vérifié :* la dernière version consolidée de l'annexe I et la réglementation française de cryptologie.

## 5. Licences des composants libres

620 crates Rust embarquées (relevé `cargo metadata`, 2026-10-04) : toutes permissives (MIT, Apache-2.0,
BSD, ISC, Zlib, Unicode-3.0…). 5 sous **MPL-2.0** (`cssparser`, `cssparser-macros`, `dtoa-short`,
`option-ext`, `selectors`, via Tauri) : utilisables sans modification, en indiquant où trouver leur source.
Double licence avec GPL/LGPL (`self_cell`, `r-efi`) : option permissive retenue. Polices Inter et JetBrains
Mono : licences jointes (`assets/fonts`).

| # | Constat | Action | Statut |
|---|---|---|---|
| L1 | Le `.deb` ne contenait **aucun avis de licences tierces** (exigé par MIT/BSD/Apache) | `tools/third_party_notices.py` génère `packaging/THIRD-PARTY-NOTICES.txt` (505 composants : crates liées au helper et à l'application sous Linux, paquets npm réellement présents dans le bundle d'après la source map de Vite, polices ; textes de licence lus dans chaque paquet, textes SPDX officiels pour les 34 paquets qui n'en contiennent pas). Installé dans `/usr/share/doc/oblivio/` ; la CI échoue s'il n'est plus à jour | **Fait** (2026-10-04) |
| L2 | Crates publiques déclarées Apache-2.0 **sans fichier `LICENSE`** | Texte officiel (apache.org, SHA-256 `cfc7749b…`) dans `public/*/LICENSE` | **Fait** (2026-10-04) |

## 6. Plan d'actions

| Priorité | Actions |
|---|---|
| Avant la première vente | C1, R5 ; validation juridique de ce document (L1, L2 faits) |
| Produit (prochaines versions) | R1, R2, R3, R4, C2 |
| Avant le 11/12/2027 | C3, C4 |
| Normes | Acquisition et analyse d'écarts IEEE 2883 et ISO/IEC 27040 (§ 7) ; comparaison avec l'annexe C du NIST ; décision « profil BSI » (§ 3.1) |
| Option commerciale | CSPN catégorie 4 |

## 7. Normes payantes : procédure

1. Le propriétaire **achète** les textes auprès de l'éditeur officiel : ISO (ou l'AFNOR, membre français
   de l'ISO) pour ISO/IEC 27040:2024 ; l'IEEE pour IEEE 2883-2022. Priorité : **IEEE 2883**, puis
   **ISO/IEC 27040**. ISO/IEC 27001 et 21964 ne sont pas nécessaires au produit.
2. Les PDF sont rangés **hors du dépôt** (droit d'auteur : jamais commités ni publiés).
3. Analyse d'écarts clause par clause, consignée ici **sans reproduire le texte** des normes : exigence →
   état d'Oblivio → action.

## 8. Sources (consultées le 2026-10-04)

- RGPD : Office des publications de l'UE, CELEX 32016R0679 — http://publications.europa.eu/resource/celex/32016R0679
- CRA : CELEX 32024R2847 — http://publications.europa.eu/resource/celex/32024R2847
- Biens à double usage : CELEX 32021R0821 — http://publications.europa.eu/resource/celex/32021R0821
- CNIL, fiche 15 : https://www.cnil.fr/fr/securite-encadrer-la-maintenance-et-la-fin-de-vie-des-materiels-et-logiciels ;
  guide 2026 : https://www.cnil.fr/sites/default/files/2026-05/cnil_guide_securite_personnelle.pdf ;
  guide du développeur : https://www.cnil.fr/fr/guide-rgpd-du-developpeur
- NIST SP 800-88 Rev. 2 : https://csrc.nist.gov/pubs/sp/800/88/r2/final
- IEEE 2883-2022 : https://standards.ieee.org/ieee/2883/10277/
- ISO/IEC 27040:2024 : https://www.iso.org/standard/80194.html ; ISO/IEC 21964 : https://www.iso.org/standard/72204.html
- BSI CON.6 (Édition 2023) : https://www.bsi.bund.de/SharedDocs/Downloads/DE/BSI/Grundschutz/IT-GS-Kompendium_Einzel_PDFs_2023/03_CON_Konzepte_und_Vorgehensweisen/CON_6_Loeschen_und_Vernichten_Edition_2023.pdf
- ANSSI PA-097 : https://messervices.cyber.gouv.fr/documents-guides/anssi-guide-reconditionnement_ordinateurs_bureau_portables_v1-0.pdf ;
  CSPN Blancco : https://messervices.cyber.gouv.fr/visas/ANSSI-CSPN-2021-04-rapport.pdf ;
  catalogue : https://messervices.cyber.gouv.fr/visas/catalogue-produits-services-profils-de-protection-sites-certifies-qualifies-agrees-anssi.pdf
