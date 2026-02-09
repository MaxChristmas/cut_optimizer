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

## Resume

- **12 tests** au total
- **5 tests** couvrent le module `guillotine` (placement, rotation, kerf, depassement, remplissage exact)
- **7 tests** couvrent le module `solver` (cas simple, remplissage exact, multi-feuilles, rotation, kerf, cas vide, calcul de chute)
