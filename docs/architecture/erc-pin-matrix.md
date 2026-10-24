# ERC pin connection matrix: axis mapping

`signex-erc::pin_matrix` has six axes, but a pin carries one of 14
`PinDirection`s. `PinAxis::of` is the single place that folds the 14
onto the six, and its table is the decision record:

| `PinDirection`                                    | axis          | why |
|---------------------------------------------------|---------------|-----|
| `Input`                                           | `Input`       | a pure sink |
| `Output`                                          | `Output`      | a push-pull driver; two on one net fight |
| `Bidirectional`                                   | `Bidir`       | |
| `ThreeStatable`, `OpenDrainLow`, `OpenDrainHigh`  | `Bidir`       | these may legally share a net (bus, wired-OR); filing them under `Output` would flag every shared bus as a short |
| `Differential`, `Clock`                           | `Bidir`       | the pin model does not say which way they drive, so they are not treated as a hard driver |
| `PowerInput`, `GroundReference`                   | `PowerIn`     | a ground pin is a supply sink whose rail is 0 V |
| `PowerOutput`                                     | `PowerOut`    | a regulator / battery output |
| `DoNotConnect`                                    | `NC`          | manufacturer-marked no-connect |
| `Passive`, `Unclassified`                         | *(none)*      | a resistor terminal can sit on any net, and an unclassified pin carries no claim to check; neither can conflict |
