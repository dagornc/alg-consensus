# ALG_CONSENSUS — spécification v7 / Rust v6

## Statut et périmètre

Cette version définit un **simulateur déterministe de diffusion du maximum**,
pas un protocole de consensus byzantin, Raft, ni un transport réseau déployable.
Elle prolonge la spécification v6 sans la remplacer rétroactivement : les
moteurs Rust v1–v5 restent disponibles, avec leur contrat historique.
Le nouveau moteur est `protocole_v6::Moteur`.

## État et fusion

30 identifiants fixes, grille 6 × 5, voisinage orthogonal. Un agent est inactif
(`None`) ou porte `(valeur: i32, horloge: u32)`. La fusion prend le maximum
lexicographique du **couple complet**, sans incrément automatique de l'horloge.
Elle est associative, commutative et idempotente. Entre événements explicites,
l'état d'un agent actif ne décroît pas. Une réintégration peut injecter un état
plus petit ; cette exception est intentionnelle et versionne son incarnation.
L'accord exige au moins un actif et l'égalité de tous les couples actifs.
C'est une observation centralisée, pas une preuve de terminaison distribuée.

## Canal unique et ordre d'une période

1. Incrémenter le tour puis appliquer les événements de ce tour dans l'ordre
   des identifiants : retrait ou injection d'une nouvelle incarnation.
2. Livrer les messages échus, en rejetant les incarnations obsolètes.
3. Photographier les états qui seront émis. Parcourir les sources puis les
   voisins dans l'ordre déterministe de `sim::voisins`, puis l'éventuelle sonde.
4. Chaque copie consomme une tentative et un tirage SplitMix64, même rejetée.
   Classer par priorité : partition, destination inactive, perte aléatoire,
   saturation, admission. Une probabilité de perte de 1 bloque donc aussi les
   sondes : aucune lecture/fusion distante ne contourne le canal.
5. Livrer les messages de latence zéro après **toutes** les émissions : pas
   de cascade instantanée dépendant de l'ordre des identifiants.
6. Actualiser les compteurs de stabilité et publier l'observation.

Une émission au tour t arrive au tour t + latence. La partition sépare les
colonnes 0–2 des colonnes 3–5 et bloque les admissions jusqu'à
`fin_partition` inclus. Les conditions du lien sont testées à l'admission ;
un retrait de la source n'annule pas un message déjà admis. Le changement
d'incarnation de la destination, lui, l'invalide à la livraison.

## Quiescence et reconnexion

Un actif émet vers ses voisins si la quiescence est désactivée, s'il vient de
changer par livraison différée, si sa stabilité est sous le seuil ou lors
d'un réveil périodique. Indépendamment, tous les `sonde` tours il émet vers
`(id + offset) mod 30`, offset parcourant 1…29. Une cible déjà voisine n'est
pas dupliquée. Zéro désactive les sondes. Aucun pair n'est sélectionné par
lecture préalable de son état ou de son activité.

Sous population finalement stable, absence de saturation, communications
récurrentes équitables et exécution illimitée, chaque actif peut recevoir le
maximum survivant via les sondes. Cela ne garantit pas la convergence dans
un horizon/budget fini, avec perte totale, ni la conservation d'un maximum
dont le dernier porteur disparaît avant toute transmission. Le PRNG fini
n'est pas une preuve d'indépendance probabiliste ni une source cryptographique.

## Optimisation du calendrier

Le mode agrégé conserve, pour chaque date d'arrivée et destination, un seul
couple maximal, l'incarnation cible et le nombre de copies admises. À latence
fixe et événements uniquement aux frontières des tours, les messages d'une
même case visent la même incarnation. L'associativité du max permet de
remplacer leurs fusions successives par une fusion unique. Le multiplicateur
préserve les compteurs exacts. Les décisions d'admission et tirages ne sont
**jamais** agrégés : ordre RNG, pertes et capacité restent identiques au mode
brut. Cette équivalence ne s'étend pas sans nouvelle preuve aux délais
variables, effets secondaires par message ou fusions non idempotentes.

Après un tour, au plus `min(latence, horizon)` lots et 30 cases par lot en
mode agrégé ; également bornés par les copies en vol. Coût d'émission
proportionnel aux tentatives, livraison proportionnelle aux cases agrégées.
La sélection des événements utilise une recherche binaire puis les seuls
événements du tour. Le mode brut est un oracle de stockage volontairement
simple (une copie dans un lot de 30 cases), **pas une baseline optimisée**.

## Comptabilité et limites

`tentatives = perdues + partition + inactif + saturation + livrees + incarnation + en_vol`.

Capacité mesurée en copies logiques, identique dans les deux modes. Même à
latence zéro, l'admission précède la livraison et peut saturer. Budget épuisé :
arrêt des émissions au message exact, fin de la période courante, puis arrêt
du moteur sans vidange forcée. À l'horizon, les messages futurs restent en vol.
L'accord ne provoque pas d'arrêt anticipé : les événements futurs sont honorés.

Validation : horizon ≤ 100 000 ; latence ≤ 10 000 ; duplication ≤ 64 ;
budget ≤ 10 millions ; capacité ≤ 100 000 ; événements ≤ 10 000 ; probabilité
finie dans [0,1] ; réveil non nul ; partition dans l'horizon. Événements
strictement ordonnés `(tour,agent)`, tours 1…horizon, identifiants 0…29.
Budget/horizon/duplication/capacité nuls sont autorisés et peuvent empêcher
toute communication. Une population vide n'est pas déclarée en accord.

## Reprise exacte

Checkpoint JSON format 1 : configuration, états, tour, RNG, stabilité,
incarnations, calendrier complet, compteurs et mode de stockage. Sérialisation
des flottants avec aller-retour exact. Reprise aux frontières des périodes,
sans reseeding et sans modification implicite des paramètres.

Chargement limité à 64 Mio ; schéma strict, limites de configuration,
conservation des compteurs, incarnations attendues, calendrier et sommes des
copies contrôlés. Ce contrôle structurel ne prouve pas l'authenticité ni
l'histoire d'un fichier forgé. Aucun secret n'est nécessaire ou enregistré.
Le CLI refuse d'écraser un fichier existant et synchronise le fichier écrit.
Il ne promet pas une transaction durable après panne du système de fichiers :
une écriture interrompue peut laisser un fichier incomplet, rejeté à la reprise.

## Validation reproductible

Voir [développement](V6_PROTOCOLE.md) et [audit](AUDIT_V6.md). Les tests
différentiels comparent le stockage brut et agrégé **à chaque tour** ; ils
partagent le protocole, donc des cas adverses indépendants sont indispensables.
Leur succès constitue une validation expérimentale bornée, jamais une preuve
d'absence universelle de défauts.
