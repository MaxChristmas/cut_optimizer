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

Routes :

| Methode | Chemin | Description |
|---|---|---|
| `GET` | `/up` | Health check, retourne `"ok"` |
| `POST` | `/optimize` | Lance l'optimisation, retourne le plan de decoupe |

---

## API — Contrat JSON

### Requete `POST /optimize`

```json
{
  "stock": {
    "length": 2400,
    "width": 1200,
    "grain": "none"
  },
  "cuts": [
    { "rect": { "length": 800, "width": 600 }, "qty": 3, "grain": "auto" },
    { "rect": { "length": 400, "width": 300 }, "qty": 5, "grain": "auto" }
  ],
  "kerf": 3,
  "cut_direction": "auto",
  "allow_rotate": true
}
```

#### Champs de la requete

| Champ | Type | Requis | Defaut | Description |
|---|---|---|---|---|
| `stock.length` | `u32` | oui | — | Longueur du panneau de stock (axe X) |
| `stock.width` | `u32` | oui | — | Largeur du panneau de stock (axe Y) |
| `stock.grain` | `string` | non | `"none"` | Sens du fil du panneau : `"none"`, `"along_length"`, `"along_width"` |
| `cuts[].rect.length` | `u32` | oui | — | Longueur de la piece |
| `cuts[].rect.width` | `u32` | oui | — | Largeur de la piece |
| `cuts[].qty` | `u32` | oui | — | Nombre d'exemplaires |
| `cuts[].grain` | `string` | non | `"auto"` | Sens du fil de la piece : `"auto"`, `"length"`, `"width"` |
| `kerf` | `u32` | non | `0` | Largeur du trait de coupe (soustrait a chaque decoupe) |
| `cut_direction` | `string` | non | `"auto"` | Direction de coupe : `"auto"`, `"along_length"`, `"along_width"` |
| `allow_rotate` | `bool` | non | `true` | Autoriser la rotation des pieces a 90 deg. |

> Les champs numeriques acceptent les nombres entiers ou les nombres flottants sans decimales (ex: `3` ou `3.0`).

#### Valeurs des enums

| Enum | Valeurs | Description |
|---|---|---|
| `stock.grain` | `none` | Pas de fil (rotation libre) |
| | `along_length` | Fil parallele a la longueur |
| | `along_width` | Fil parallele a la largeur |
| `cuts[].grain` | `auto` | Pas de contrainte de fil sur la piece |
| | `length` | Le cote "length" de la piece doit s'aligner avec le fil du stock |
| | `width` | Le cote "width" de la piece doit s'aligner avec le fil du stock |
| `cut_direction` | `auto` | Teste les deux directions, garde la meilleure |
| | `along_length` | Coupes horizontales, pieces orientees longueur >= largeur |
| | `along_width` | Coupes verticales, pieces orientees largeur >= longueur |

#### Validations (erreurs 400)

- `stock.length` et `stock.width` doivent etre > 0.
- `cuts[].rect.length` et `cuts[].rect.width` doivent etre > 0.
- `cuts[].qty` doit etre > 0.
- Chaque piece doit rentrer dans le stock (en tenant compte de la rotation et du grain). Sinon : `"piece LxW does not fit in stock LxW"`.

### Reponse `POST /optimize`

```json
{
  "stock": { "length": 2400, "width": 1200 },
  "sheet_count": 2,
  "waste_percent": 47.2,
  "sheets": [
    {
      "waste_area": 1528000,
      "placements": [
        {
          "rect": { "length": 1000, "width": 500 },
          "x": 0,
          "y": 0,
          "rotated": true
        },
        {
          "rect": { "length": 800, "width": 600 },
          "x": 502,
          "y": 0,
          "rotated": false
        }
      ]
    }
  ]
}
```

#### Champs de la reponse

| Champ | Type | Description |
|---|---|---|
| `stock` | `Rect` | Dimensions du panneau de stock utilise |
| `sheet_count` | `usize` | Nombre total de panneaux utilises |
| `waste_percent` | `f64` | Pourcentage de chute global (0-100) |
| `sheets[]` | `array` | Liste des panneaux avec leurs placements |
| `sheets[].waste_area` | `u64` | Surface de chute sur ce panneau (stock_area - somme des pieces) |
| `sheets[].placements[]` | `array` | Liste des pieces placees sur ce panneau |
| `sheets[].placements[].rect` | `Rect` | Dimensions de la piece **telle que placee** (apres rotation eventuelle) |
| `sheets[].placements[].x` | `u32` | Position X sur le panneau (axe longueur, depuis le bord gauche) |
| `sheets[].placements[].y` | `u32` | Position Y sur le panneau (axe largeur, depuis le bord haut) |
| `sheets[].placements[].rotated` | `bool` | `true` si la piece a ete tournee de 90 deg. par rapport a la demande |

> `rect` dans la reponse contient les dimensions **apres rotation** : si `rotated: true`, length et width sont inverses par rapport a la demande d'origine.

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
  |-- Phase 1 : Greedy (3 strategies x 2 directions, garde la meilleure)
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

#### Departage a nombre de panneaux egal

Quand plusieurs strategies/directions produisent le meme nombre de panneaux, le solveur prefere la solution dont le **dernier panneau a la bounding box la plus compacte** (plus petite surface englobante des pieces placees). Cela evite les dispositions en L peu pratiques et favorise des placements alignes sur le dernier panneau.

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
Piece 1 -> meilleur espace -> ok   Piece 1 -> essai orientation A
Piece 2 -> meilleur espace -> ok     Piece 2 -> dans panneau 1 -> ok
Piece 3 -> ne rentre pas           Piece 3 -> ne rentre pas -> RETOUR
        -> nouveau panneau              Piece 2 -> essai orientation B
                                         Piece 3 -> rentre ! -> 1 panneau
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

### Systeme de coordonnees

- **x** : axe longueur (horizontal), de gauche a droite.
- **y** : axe largeur (vertical), de haut en bas.
- L'origine `(0, 0)` est en haut a gauche du panneau.
- Une piece placee a `(x, y)` occupe la zone `[x, x+length) x [y, y+width)`.

### Conventions sur les dimensions

- `length` = dimension le long de l'axe X (horizontal).
- `width` = dimension le long de l'axe Y (vertical).
- Quand `rotated: true`, length et width sont inverses : la piece d'origine `{length: L, width: W}` est placee comme `{length: W, width: L}`.

## Developpement

```bash
cargo build              # Build debug
cargo build --release    # Build release
cargo test               # Lancer tous les tests
cargo clippy             # Linter
cargo fmt                # Formater le code
```
