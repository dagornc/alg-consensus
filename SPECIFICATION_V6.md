# ALG_CONSENSUS — spécification v6

## 1. Périmètre et versions

Cette spécification accompagne le **développement Rust v5**, branche `v5`,
succédant à `v4` (`58d59f6`). La numérotation de la spécification est distincte
de celle du logiciel. La v5 antérieure de la spécification est référencée par
le README et le simulateur Python embarqué ; son document intégral n'est pas
présent dans ce dépôt. La v6 ci-dessous formalise le périmètre effectivement
implémenté, sans prétendre reproduire les sections absentes.

Objectif : diminuer le coût CPU et le stockage de messages du **simulateur**,
sans modifier sa trajectoire observable ni ses tirages pseudo-aléatoires.
Ce n'est ni un consensus byzantin, ni un registre linéarisable, ni une pile
réseau de production. Aucun gain de bande passante n'est revendiqué : les
compteurs de messages doivent rester identiques.

## 2. État et algèbre

Un état est `Option<(i32,u32)>`. `None = ⊥` est l'élément **neutre** (et non
absorbant, contrairement à un commentaire historique). L'ordre des couples
est lexicographique, priorité à la valeur. La jointure est le maximum.

- Neutralité : `a ⊔ ⊥ = a`.
- Idempotence : `a ⊔ a = a`.
- Commutativité : `a ⊔ b = b ⊔ a`.
- Associativité : `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`.
- Inflation : `a ≤ a ⊔ b` et `b ≤ a ⊔ b`.

Ces propriétés suivent de l'ordre total avec minimum ajouté. Elles rendent
une fusion de messages reçus indépendante de leur ordre et de leur multiplicité.
Elles ne garantissent **pas** que les messages nécessaires seront reçus.
Une convergence éventuelle exige notamment la circulation persistante de
l'information sur une topologie temporelle adéquate, sans perte permanente.
Un réveil périodique local ne démontre pas à lui seul cette propriété.

Les horloges sont initialisées à zéro et ne sont pas incrémentées par le
simulateur actuel. Le critère historique d'accord compare **les valeurs**,
pas les couples complets ; il doit rester nommé et interprété comme tel.

## 3. Configuration et observation

Paramètres publics inchangés : `Params` (grille 6×5, 30 agents, fanout 6,
perte i.i.d., duplication, retrait, partition, latence fixe en périodes,
divergence forcée, reconnexion et quiescence optionnelles).

La v5 exige une perte finie dans `[0,1]` et rejette un horizon `usize::MAX`
ou une borne `N × FANOUT × duplication × max_periodes` supérieure à `u64::MAX`.
Le calcul de cette borne utilise des multiplications vérifiées en `u128`.
Ce contrôle arithmétique n'est **pas** une protection contre une charge excessive.
Un service accueillant des paramètres externes doit imposer des quotas CPU,
temps et mémoire supplémentaires ; cette version ne constitue pas ce service.

Cas définis : horizon zéro = état initial observé ; duplication zéro = aucun
message du canal normal ; retrait plafonné pour conserver un actif ; retrait
programmé après l'arrêt jamais exécuté ; `t_retrait=Some(0)` non exécuté,
conformément à la référence. Les options de reconnexion idéalisée peuvent
encore propager des états avec duplication zéro (voir §7).

## 4. Ordre d'une période

Pour `t=1..max_periodes`, conserver strictement l'ordre historique :

1. Appliquer le retrait programmé, avec le même échantillonnage RNG.
2. Livrer les messages d'arrivée `≤t`.
3. Pour chaque agent actif, dans l'ordre des identifiants : construire les
   cibles dans l'ordre gauche/droite/haut/bas, filtrer la partition puis les
   retraits si reconnexion, appliquer les règles historiques de reconnexion.
4. Appliquer la quiescence ; si émission, appeler le même `sample`, puis le
   même `random` pour chaque copie. Compter chaque tentative même perdue.
5. Fusionner les messages immédiats **après** toutes les émissions.
6. Mettre à jour les compteurs de stabilité.
7. Mesurer la divergence à la fin de partition puis l'accord en valeur.
8. Arrêter dès le premier accord comme la référence, même si des événements
   futurs sont encore programmés. Sinon terminer à l'horizon.

Sorties : tous les champs de `Resultat`, états finaux optionnels, activité,
quiescence. Le booléen de quiescence décrit la décision basée sur le compteur
final, pas une trace certaine de l'émission de la dernière période.

## 5. Optimisations normatives

### 5.1 Jointure par destinataire et instant d'arrivée

La file d'un destinataire est une `VecDeque<(arrivee,etat)>`. La latence étant
fixe, les arrivées sont monotones. Toutes les copies **acceptées par le canal**
destinées au même agent et à la même période sont remplacées par leur jointure.
Les tirages de perte et le comptage ont lieu **avant** ce regroupement.

Justification : à la livraison, la jointure de la jointure est égale à la
jointure des messages originaux par associativité et idempotence. Aucun état
intermédiaire n'est observé entre les livraisons d'une même phase. Par induction
sur les périodes, les phases suivantes reçoivent donc les mêmes états et le
RNG reste au même point. Les choix, compteurs et sorties sont identiques.

Ne pas regrouper des arrivées différentes : cela avancerait la disponibilité
d'une information et pourrait modifier les transmissions et l'arrêt.
Ne pas supprimer les tirages associés aux copies dominées.

### 5.2 Horizon et arithmétique

Un message n'est stocké que si `latence <= max_periodes - t`. Alors seulement
`t + latence` est calculé. Un message postérieur à l'horizon ne peut influencer
aucune sortie, mais sa tentative reste comptée et son tirage de perte conservé.
Cela définit aussi proprement des latences pour lesquelles la référence
déborderait ; on ne revendique pas une parité avec un comportement débordant.

### 5.3 Buffers et test d'accord

Pré-calculer le voisinage, réutiliser le buffer de cibles, utiliser un tableau
de 30 cases pour les nouveaux états. Tester l'unicité par parcours avec arrêt
au premier désaccord, sans `HashSet`. Un ensemble vide n'est pas un accord.
Le découpage gauche/droite conserve exactement le test de divergence.

### 5.4 Complexité

Avec `H` périodes, latence `L>0`, fanout borné `F`, duplication `D` : la
référence peut conserver `O(N F D min(L,H))` messages et les rescanner à chaque
période. La v5 conserve au plus un agrégat par destinataire/date, donc
`O(N min(L,H))` agrégats ; chaque agrégat est livré une fois. Les tentatives
restent `O(H N F D)` et les recherches de reconnexion inchangées. Pour `L=0`,
aucune file différée. Ce sont des bornes structurelles, pas une mesure RSS.

## 6. Compatibilité et interface

`optimise_v5::simuler_v5` et `simuler_avec_vues_v5` retournent `Result`.
`--v5` sélectionne le moteur, `--v5 --v3` ajoute les politiques v2/v3.
`--v4 --v5` est rejeté : la couche opérationnelle v4 reste indépendante et
n'est pas accélérée implicitement. `src/sim.rs` et la référence Python sont
conservés. Les CSV existants et les API historiques ne changent pas.

## 7. Limites héritées, explicitement non corrigées

- Les deux blocs de réciprocité/reconnexion fusionnent directement des états,
  sans perte, délai ni incrément du compteur : **canal idéalisé**, pas trafic réel.
- Les agents retirés peuvent encore recevoir des états internes ; ils ne
  participent plus à l'accord et leur vue exportée est `None`.
- Le premier accord interrompt les retraits futurs et les futures partitions.
- L'accord ne prouve ni fraîcheur, ni autorisation, ni robustesse byzantine.
- La sérialisation opérationnelle v4 n'est pas un checkpoint complet : elle
  n'enregistre pas le RNG, la file, ni tous les compteurs pour reprendre ce run.
- Aucun temps de convergence borné n'est démontré sous pertes arbitraires.
- La topologie est fixe ; les benchmarks ne prouvent pas une mise à l'échelle.

Une correction de ces choix exigerait un mode protocole séparé, avec toutes
les communications explicites et un oracle différent. Elle ne doit pas être
introduite discrètement dans une optimisation censée préserver la parité.

## 8. Critères de validation

1. Suite historique intacte ; tests Rust en debug et release.
2. Comparaison différentielle de tous les champs et vues sur partitions,
   pertes extrêmes, délais, duplications, retraits et politiques v2/v3.
3. Vérification Python indépendante pour le profil historique supporté.
4. Tests négatifs des pertes invalides et débordements.
5. Benchmark release apparié avec échauffement, ordre alterné, sept mesures,
   médiane et description de l'environnement ; aucun seuil temporel fragile en CI.

Une suite verte signifie « aucun défaut détecté dans ces contrôles », pas
« aucune faille possible ». Voir `V5_OPTIMISATION.md` pour les résultats et
`AUDIT_V5.md` pour la trace des itérations.
