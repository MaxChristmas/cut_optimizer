# Cut Optimizer

Outil en Rust pour l'optimisation de decoupe 2D de panneaux rectangulaires. Minimise le nombre de panneaux utilises et le pourcentage de chute.

## Installation

```bash
cargo build --release
```

Le binaire se trouve dans `target/release/cut_optimizer`.

## Utilisation

### CLI

```bash
cut_optimizer --stock <LxW> --cuts <LxW:qte> [<LxW:qte> ...] [options]
```

### Options

| Option | Description | Defaut |
|---|---|---|
| `--stock <LxW>` | Dimensions du panneau de stock (ex: `2400x1200`) | **requis** |
| `--cuts <LxW:qte>` | Pieces a decouper avec quantite (ex: `800x600:3`) | **requis** |
| `--kerf <N>` | Largeur du trait de coupe en mm | `0` |
| `--no-rotate` | Desactiver la rotation des pieces a 90 deg. | rotation activee |
| `--cut-direction <dir>` | Direction de coupe : `auto`, `along-length`, `along-width` | `auto` |
| `--layout` | Afficher un schema ASCII de chaque panneau | desactive |

### Exemples

```bash
# Decoupe basique
cut_optimizer --stock 2400x1200 --cuts 800x600:3 400x300:5 1000x500:2

# Avec trait de coupe (kerf)
cut_optimizer --stock 2400x1200 --kerf 3 --cuts 800x600:3 400x300:5

# Sans rotation, avec schema ASCII
cut_optimizer --stock 2400x1200 --no-rotate --layout --cuts 800x600:3
```

### Serveur HTTP

Un serveur HTTP (axum) est egalement disponible pour une utilisation via API :

```bash
cargo run --bin server
# Ecoute sur 0.0.0.0:3001 (configurable via $PORT)
```

Endpoint `POST /optimize` avec un body JSON :

```json
{
  "stock": { "length": 2400, "width": 1200, "grain": "none" },
  "cuts": [
    { "rect": { "length": 800, "width": 600 }, "qty": 3, "grain": "auto" },
    { "rect": { "length": 400, "width": 300 }, "qty": 5, "grain": "auto" }
  ],
  "kerf": 3,
  "cut_direction": "auto",
  "allow_rotate": true
}
```

### Format de sortie (CLI)

```
Sheet 1:
  500x1000 @ (0, 0) [rotated]
  800x600 @ (1000, 0)
  300x400 @ (1000, 800) [rotated]

Sheet 2:
  300x400 @ (0, 0) [rotated]

Summary: 2 sheets used, 47.2% waste
```

Chaque ligne indique : dimensions de la piece, position `(x, y)` sur le panneau, et `[rotated]` si la piece a ete tournee de 90 deg.

---

## Fonctionnement de l'algorithme

### Vue d'ensemble

```
Entree (stock + pieces + options)
  |
  v
Contraintes de rotation (grain + direction de coupe)
  |
  v
Solver
  |-- Phase 1 : Greedy (3 strategies, garde la meilleure)
  |-- Phase 2 : Branch & Bound (amelioration, <= 20 pieces)
  |
  v
Solution (panneaux + placements + % de chute)
```

### Etape 1 — Contraintes de rotation

Avant de placer quoi que ce soit, chaque piece recoit une contrainte de rotation :

- **Free** : la piece peut etre placee dans les deux orientations.
- **NoRotate** : la piece doit rester dans son orientation d'origine.
- **ForceRotate** : la piece doit etre tournee de 90 deg.

Cette contrainte depend de deux facteurs :

- **Le fil du bois (grain)** : si le panneau de stock a un fil (ex: `along_length`) et que la piece aussi (ex: `grain: length`), la rotation est soit interdite (alignement naturel) soit forcee (alignement croise) pour respecter le sens du fil.
- **La direction de coupe** : si `along-length` est demandee, les pieces sont orientees pour que leur cote le plus long soit aligne avec la longueur du panneau (et inversement pour `along-width`).

Le grain a priorite sur la direction de coupe.

### Etape 2 — Phase greedy

Les pieces sont triees par **aire decroissante** (les plus grandes d'abord), puis placees une par une.

Pour chaque piece, le solveur cherche le meilleur espace libre parmi tous les panneaux ouverts. S'il n'en trouve pas, il ouvre un nouveau panneau.

Le choix du "meilleur" espace depend de la **strategie de scoring**. Le solveur essaie les 3 strategies et garde la solution qui utilise le moins de panneaux :

| Strategie | Critere principal | Comportement |
|---|---|---|
| **BestAreaFit** | Plus petite difference de **surface** entre l'espace libre et la piece | Prefere l'espace dont la taille est la plus proche de la piece |
| **BestShortSideFit** | Plus petit **meilleur ecart** de cote | Prefere l'espace ou la piece est bien serree sur au moins un cote, meme si l'autre cote a un gros ecart |
| **BestLongSideFit** | Plus petit **pire ecart** de cote | Prefere l'espace ou le pire ecart est le moins mauvais |

Pour chaque espace libre, les deux orientations de la piece (normale et tournee a 90 deg.) sont testees si la rotation est autorisee. Le meilleur score gagne.

En mode `auto` pour la direction de coupe, les directions `along-length` et `along-width` sont egalement testees, ce qui donne jusqu'a 6 variantes (3 strategies x 2 directions).

### Etape 3 — Guillotine Bin Packing

C'est le moteur de placement 2D. Chaque panneau est gere comme un ensemble de **rectangles libres**.

Quand une piece est placee dans un espace libre :

1. L'espace est **divise en 2 rectangles** par une coupe qui traverse tout l'espace d'un bord a l'autre (coupe guillotine), comme une vraie scie sur un panneau.
2. La direction de la coupe depend de `cut_direction` :
   - `along-length` : coupe horizontale (le rectangle du bas prend toute la longueur).
   - `along-width` : coupe verticale (le rectangle de droite prend toute la largeur).
   - `auto` : coupe sur l'axe qui a le plus petit residu.
3. Les rectangles libres adjacents sont **fusionnes** quand c'est possible pour reduire la fragmentation.

Exemple de coupe guillotine :

```
Espace 200x150, piece 80x60 placee en haut a gauche

Coupe horizontale :             Coupe verticale :
+--------+----------+           +--------+---------+
| piece  | 120x60   |           | piece  |         |
+--------+----------+           +--------+ 120x150 |
|     200x90        |           | 80x90  |         |
+-------------------+           +--------+---------+
```

Le kerf (largeur de lame) est soustrait a chaque coupe : un espace de 200 avec une piece de 80 et un kerf de 3 donne un residu de 200 - 80 - 3 = 117.

### Greedy vs Branch & Bound

Les deux phases utilisent la guillotine de la meme facon pour placer et decouper. La difference est dans **comment ils decident ou mettre chaque piece** :

- **Greedy** : pour chaque piece, il prend la meilleure option immediate et ne revient jamais en arriere. Rapide, mais parfois sous-optimal — un mauvais placement tot peut forcer l'ouverture d'un panneau supplementaire plus tard.
- **Branch & Bound** : il explore toutes les possibilites (quelle piece dans quel panneau, dans quelle orientation) et revient en arriere si le resultat n'est pas bon. Il garde le meilleur resultat global.

```
Greedy :                          Branch & Bound :
Piece 1 → meilleur espace → ok   Piece 1 → essai orientation A
Piece 2 → meilleur espace → ok     Piece 2 → dans panneau 1 → ok
Piece 3 → ne rentre pas           Piece 3 → ne rentre pas → RETOUR
        → nouveau panneau              Piece 2 → essai orientation B
                                         Piece 3 → rentre ! → 1 panneau
```

| | Greedy | Branch & Bound |
|---|---|---|
| **Choix** | Le meilleur **maintenant** | Le meilleur **globalement** |
| **Retour en arriere** | Non | Oui |
| **Vitesse** | Rapide (lineaire) | Lent (exponentiel, limite a 20 pieces) |
| **Resultat** | Bon | Potentiellement meilleur |

### Etape 4 — Branch & Bound (amelioration)

Active uniquement pour **20 pieces ou moins** (au-dela le cout est trop eleve).

Le greedy a trouve une solution en N panneaux. Le Branch & Bound essaie de trouver une solution en N-1 panneaux ou moins en explorant un arbre de decisions :

- A chaque noeud : une piece a placer.
- Branches possibles : la placer dans chacun des panneaux existants (dans chaque orientation), ou ouvrir un nouveau panneau.
- L'algorithme explore en profondeur (DFS) et coupe les branches inutiles grace a 3 regles d'elagage :

| Regle | Condition | Effet |
|---|---|---|
| **Borne superieure** | Le nombre de panneaux ouverts >= meilleure solution connue | Coupe la branche |
| **Borne inferieure** | Surface restante des pieces / surface libre disponible >= meilleure solution | Coupe la branche |
| **Nouveau panneau** | Ouvrir un panneau ferait atteindre la meilleure solution | N'ouvre pas |

Des qu'une meilleure solution est trouvee, la borne superieure se resserre et l'elagage devient encore plus agressif.

Si le B&B trouve une solution utilisant moins de panneaux que le greedy, elle la remplace. Sinon, le resultat greedy est conserve.

---

## Architecture

```
src/
  main.rs          # CLI (clap) : parsing, validation, affichage
  bin/server.rs    # Serveur HTTP (axum) : API REST POST /optimize
  lib.rs           # Point d'entree de la bibliotheque
  types.rs         # Structures (Rect, Demand, Placement, Solution, RotationConstraint)
  solver.rs        # Solveur : greedy (3 strategies) + Branch & Bound
  guillotine.rs    # Moteur de placement 2D (split, merge, scoring)
  render.rs        # Rendu ASCII des panneaux
```

## Developpement

```bash
cargo build              # Build debug
cargo build --release    # Build release
cargo test               # Lancer tous les tests
cargo clippy             # Linter
cargo fmt                # Formater le code
```
