# ML — Memphis Language

Experimental language for hardware control (gates 0/1 + temperature sensors).

## Building

```bash
cargo build -p ml-core
```

## Running

```bash
cargo run -p ml-core --example run -- "(gate garage on)"
cargo run -p ml-core --example run -- "(read temp.living_room)"
cargo run -p ml-core --example run -- "(if (> temp.server 40) (gate fan on) (gate fan off))"
```

## Architecture

```
crates/ml-core/
├── Cargo.toml
├── examples/
│   └── run.rs           ← CLI example
└── src/
    ├── lib.rs           ← public API
    ├── ast.rs           ← MLExpr enum
    ├── error.rs         ← ParseError, RuntimeError
    ├── lexer.rs         ← Logos tokenizer
    ├── machine.rs       ← Machine trait + MockMachine
    └── parser.rs        ← recursive descent parser
```

## Language Reference

### Gate control
```
(gate <id> on)     — włącz bramkę
(gate <id> off)    — wyłącz bramkę
(gate <id> toggle) — przełącz stan
```

### Sensor reading
```
(read temp.<location>)  — odczytaj temperaturę
```

### Control flow
```
(if <condition> <then>)
(if <condition> <then> <else>)
```

### Conditions
```
(> <value> <value>)   — większe
(< <value> <value>)    — mniejsze
(== <value> <value>)   — równe
(and <cond> <cond>)    — i
(or <cond> <cond>)     — lub
(not <cond>)           — negacja
```

### Other
```
(wait <ms>)           — czekaj
(log "message")        — log
(let <name> <expr> <body>)  — zmienna lokalna
```

## Examples

```scheme
;; Włącz bramkę, poczekaj, sprawdź temperaturę
(gate garage on)
(wait 1000)
(read temp.living_room)

;; Jeśli gorąco — włącz wentylator
(if (> temp.server 40)
    (gate fan on)
    (gate fan off))

;; Sekwencja z logiem
(gate gate1 on)
(log "Gate 1 is open")
(wait 2000)
(gate gate1 off)
```

## Next

- [ ] Add GPIO backend (Raspberry Pi / ESP32)
- [ ] Add HTTP backend (REST APIs)
- [ ] Add MQTT backend (IoT sensors)
- [ ] Add loop expressions
- [ ] Add real ML → MP translation (for Memphis federation)
