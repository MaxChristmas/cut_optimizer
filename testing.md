# Tests

Tous les tests se lancent avec :

```bash
cargo test
```

## Liste des tests

### `guillotine::tests` — Module de bin packing guillotine

| Test | Description | Verification |
|---|---|---|
| `test_place_single_piece` | Place une piece 50x30 dans un bin 100x100 | Position (0,0), dimensions correctes, rectangles libres restants |
| `test_piece_too_large` | Piece 200x50 dans un bin 100x100 | `find_best` retourne `None` |
| `test_rotation_fit` | Piece 50x100 dans un bin 100x50 | Echoue sans rotation, reussit avec rotation activee |
| `test_kerf` | Place une piece 50x100 dans un bin 100x100 avec kerf=5 | Le rectangle libre restant a une largeur de 45 (100 - 50 - 5) |
| `test_fill_exact` | Piece 100x100 remplissant exactement un bin 100x100 | Aucun rectangle libre apres placement |

### `solver::tests` — Module du solveur (greedy + Branch and Bound)

| Test | Description | Verification |
|---|---|---|
| `test_single_piece` | Une seule piece 50x50 dans un stock 100x100 | 1 feuille utilisee, 1 placement |
| `test_exact_fit_four_pieces` | 4 pieces de 50x50 dans un stock 100x100 | 1 seule feuille (remplissage exact) |
| `test_needs_two_sheets` | 4 pieces de 60x60 dans un stock 100x100 | Au moins 4 feuilles (chaque piece occupe une feuille entiere) |
| `test_rotation_helps` | Piece 50x100 dans un stock 100x50 avec rotation | 1 feuille, la piece est marquee `rotated` |
| `test_no_demands` | Aucune piece demandee | 0 feuilles |
| `test_kerf_reduces_capacity` | 2 pieces de 50x100 dans un stock 100x100, sans kerf puis avec kerf=5 | Sans kerf : 1 feuille. Avec kerf : 2 feuilles (50 + 5 + 50 = 105 > 100) |
| `test_waste_percent` | Piece 100x100 remplissant exactement un stock 100x100 | Pourcentage de chute = 0% |

### `solver::tests` — Tests complexes (30+ pieces, 5-10 tailles)

| Test | Description | Verification |
|---|---|---|
| `test_complex_mixed_sizes_no_kerf` | 30 pieces, 6 tailles differentes, stock 2440x1220, sans kerf | 30 placements, chaque piece dans les limites du stock, nombre de feuilles >= borne inferieure theorique |
| `test_complex_mixed_sizes_with_kerf` | 35 pieces, 7 tailles differentes, stock 2440x1220, kerf=3 | 35 placements, chaque piece dans les limites du stock |
| `test_complex_no_rotation` | 40 pieces, 8 tailles differentes, stock 2440x1220, rotation desactivee | 40 placements, puis comparaison avec rotation activee (doit utiliser <= feuilles) |
| `test_complex_large_batch_mixed_rotation` | 50 pieces, 10 tailles differentes, stock 3000x1500, kerf=4, rotation mixte | 50 placements, limites du stock respectees, chute entre 0% et 100% |
| `test_complex_small_stock_many_sheets` | 32 pieces, 5 tailles differentes, petit stock 500x400 | 32 placements, au moins 5 feuilles, limites du stock respectees |

## Resume

- **20 tests** au total
- **5 tests** couvrent le module `guillotine` (placement, rotation, kerf, depassement, remplissage exact)
- **7 tests** couvrent le module `solver` — cas simples (cas unitaire, remplissage exact, multi-feuilles, rotation, kerf, cas vide, calcul de chute)
- **5 tests** couvrent le module `solver` — cas complexes (30-50 pieces, 5-10 tailles, kerf, rotation mixte, petit stock)
- **3 tests** couvrent le module `render` (piece unique, deux pieces, stock vide)
