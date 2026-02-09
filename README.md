# Cut Optimizer

Outil CLI en Rust pour l'optimisation de decoupe 2D de panneaux rectangulaires. Minimise le nombre de panneaux de stock utilises lors de la decoupe de pieces plus petites.

## Installation

```bash
cargo build --release
```

Le binaire se trouve dans `target/release/cut_optimizer`.

## Utilisation

### Syntaxe

```bash
cut_optimizer --stock <LxH> --cuts <LxH:qte> [<LxH:qte> ...] [--kerf <N>] [--no-rotate]
```

### Options

| Option | Description | Defaut |
|---|---|---|
| `--stock <LxH>` | Dimensions du panneau de stock (ex: `2400x1200`) | **requis** |
| `--cuts <LxH:qte>` | Pieces a decouper avec quantite (ex: `800x600:3`) | **requis** |
| `--kerf <N>` | Largeur du trait de coupe en mm | `0` |
| `--no-rotate` | Desactiver la rotation des pieces a 90 deg. | rotation activee |

### Exemples

Decoupe basique :

```bash
cut_optimizer --stock 2400x1200 --cuts 800x600:3 400x300:5 1000x500:2
```

Avec trait de coupe (kerf) :

```bash
cut_optimizer --stock 2400x1200 --kerf 3 --cuts 800x600:3 400x300:5
```

Sans rotation des pieces :

```bash
cut_optimizer --stock 2400x1200 --no-rotate --cuts 800x600:3
```

### Format de sortie

```
Sheet 1:
  500x1000 @ (0, 0) [rotated]
  500x1000 @ (500, 0) [rotated]
  600x800 @ (1000, 0) [rotated]
  800x600 @ (1600, 0)
  300x400 @ (1000, 800) [rotated]

Sheet 2:
  300x400 @ (0, 0) [rotated]
  300x400 @ (0, 400) [rotated]

Summary: 2 sheets used, 47.2% waste
```

Chaque ligne indique les dimensions de la piece, sa position `(x, y)` sur le panneau, et `[rotated]` si la piece a ete tournee de 90 deg.

## Algorithme

Le solveur combine deux approches :

1. **Heuristiques greedy** — trois strategies de scoring sont testees en parallele (Best Area Fit, Best Short Side Fit, Best Long Side Fit). La meilleure solution est conservee comme borne superieure.

2. **Branch and Bound** — exploration exhaustive de l'arbre de placement avec elagage base sur la borne superieure du greedy et sur une borne inferieure calculee a partir des surfaces restantes. Active pour les entrees de 20 pieces ou moins.

Les deux phases utilisent le **guillotine packing** : chaque placement divise l'espace libre restant en deux rectangles par une coupe bord-a-bord (guillotine), ce qui reflete les contraintes reelles des machines de decoupe.

## Architecture

```
src/
  main.rs          # Parsing CLI (clap), validation, affichage
  types.rs         # Structures de donnees (Rect, Demand, Placement, Solution)
  guillotine.rs    # Bin guillotine (rectangles libres, split, merge, scoring)
  solver.rs        # Solveur greedy + Branch and Bound
```

## Developpement

```bash
cargo build              # Build debug
cargo build --release    # Build release
cargo test               # Lancer tous les tests
cargo clippy             # Linter
cargo fmt                # Formater le code
```
