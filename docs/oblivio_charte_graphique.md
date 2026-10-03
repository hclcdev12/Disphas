# Oblivio — Charte Graphique & Identité Visuelle

Ce document formalise l'identité visuelle d'**Oblivio**. Il définit la palette de couleurs, la typographie, les formes et les règles visuelles à appliquer à l'application desktop, aux certificats PDF et aux supports de communication.

## 1. ADN & Positionnement Visuel

* **Mots-clés :** Amnésie matérielle, rigueur chirurgicale, souveraineté, autorité irréfutable, sobriété.

* **Esthétique :** Sombre, industrielle et haut de gamme. L'image de marque doit inspirer une confiance absolue (sécurité, audit) en évitant les codes ludiques ou l'esthétique datée des utilitaires système classiques.

## 2. Palette Chromatique Officielle

| Rôle | Nom de teinte | Code HEX | Usage principal | 
 | ----- | ----- | ----- | ----- | 
| **Fond Principal** | Night Slate | `#090D16` | Arrière-plan global, ambiance mate. | 
| **Surface / Cartes** | Dark Charcoal | `#111827` | Conteneurs, blocs disques, modales. | 
| **Bordure / Structure** | Steel Slate | `#1F2937` | Séparateurs, bordures de cartes (`1px`). | 
| **Texte Principal** | Soft White | `#F9FAFB` | Titres, métriques clés, informations critiques. | 
| **Texte Secondaire** | Muted Ash | `#9CA3AF` | Légendes, libellés secondaires, métadonnées. | 
| **Accent Conforme** | Emerald Green | `#10B981` | Validation *NIST Purge/Clear*, état sain. | 
| **Accent Alerte** | Crimson Red | `#EF4444` | Politique *Say NO*, destruction physique requise. | 
| **Accent Marque** | Deep Indigo | `#6366F1` | Actions principales, signature d'audit Ed25519, logo. | 

## 3. Typographie & Hiérarchie

### Titres & Interface (Sans-Serif)

* **Police :** `Plus Jakarta Sans` ou `Inter`

* **Usages :** Titres, navigation, boutons, messages d'état.

* **Graisses :** Semi-Bold (`600`) pour les titres, Medium (`500`) pour l'UI active.

### Données Techniques & Métadonnées (Monospace)

* **Police :** `JetBrains Mono` ou `Fira Code`

* **Usages :** Numéros de série (S/N), nœuds I/O (`/dev/nvme0n1`), empreintes SHA-256 / Ed25519, logs système.

* **Graisses :** Regular (`400`).

## 4. Formes & Langage Visuel

* **Rayon de Courbure (Radius) :** Bords légèrement adoucis (`rounded-lg` / 8px max). Pas de formes trop arrondies ni d'angles droits stricts.

* **Éclairage & Relief :** Effet métallique discret via un rehaussement de bordure supérieure (`inner border highlight`), sans ombres portées lourdes.

* **Iconographie :** Lignes vectorielles épurées (épaisseur de trait `1.5px` à `2px`), géométriques et sans remplissage superflu.

## 5. Applications Pratiques

* **Application Desktop (Tauri) :** Fond sombre mat `#090D16`, cartes composées en `#111827` avec bordures `#1F2937`.

* **Certificat d'Audit (PDF) :** Adaptation haute lisibilité (fond blanc `#FFFFFF`, structures `#111827`, accents Indigo et Émeraude pour la signature et le statut).