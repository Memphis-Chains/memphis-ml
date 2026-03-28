# ML — MASTER PLAN
# Memphis Language v1.0
# Wodzu + Memphisek + Iskra + Multi-Agent Swarm
# 2026-03-28

## ZAŁOŻENIA (ambitne)

1. ML = pełny język programowania (zmienne, funkcje, pętle, rekurencja, lambdy)
2. Memphis Rust Core = executor ML
3. ML = rozproszony (wiele maszyn jednocześnie, komunikacja peer-to-peer)
4. Hardware: GPIO (bramki 0/1) + sensory (temp) + HTTP APIs + MQTT + więcej
5. Memphis chains = audit trail (journal + decisions za każdą akcją ML)
6. ML ↔ MP (Memphis Protocol) = agenci gadają w ML przez Matrix transport
7. ML ↔ Memphis = Memphis może spawnować i sterować ML programami

## ARCHITEKTURA CELOWA

```
┌──────────────────────────────────────────────────────┐
│  ML Language                                         │
│  ┌────────────────────────────────────────────────┐ │
│  │ S-expressions + pełny język                    │ │
│  │ (zmienne, funkcje, pętle, lambdy, importy)    │ │
│  └────────────────────────────────────────────────┘ │
│                          ↓ parse                    │
│  ┌────────────────────────────────────────────────┐ │
│  │ AST (typed, with types)                        │ │
│  └────────────────────────────────────────────────┘ │
│                          ↓ typecheck                │
│  ┌────────────────────────────────────────────────┐ │
│  │ Typed AST + Optimized IR                        │ │
│  └────────────────────────────────────────────────┘ │
│                          ↓ compile                  │
│  ┌────────────────────────────────────────────────┐ │
│  │ ML Bytecode (stack-based VM)                   │ │
│  └────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
                           ↓ execute
┌──────────────────────────────────────────────────────┐
│  Memphis Runtime (Rust)                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐     │
│  │ ML VM      │  │ HAL        │  │ Chains     │     │
│  │ (stack)    │  │ (GPIO/HTTP │  │ (journal   │     │
│  │            │  │  /MQTT)   │  │  /decisions│     │
│  └────────────┘  └────────────┘  └────────────┘     │
│                                                    │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐     │
│  │ Network    │  │ Peer Comm │  │ Memphis    │     │
│  │ (TCP/UDP)  │  │ (ML-P2P)  │  │ Protocol   │     │
│  └────────────┘  └────────────┘  └────────────┘     │
└──────────────────────────────────────────────────────┘
```

## CRATES (nowy Memphis-like monorepo)

```
ml/
├── crates/
│   ├── ml-core          # Parser, AST, VM, Runtime (TERAZ)
│   ├── ml-lang          # Pełny język (funkcje, pętle, typy)
│   ├── ml-hw-gpio       # GPIO backend (Raspberry Pi, ESP32)
│   ├── ml-hw-http       # HTTP sensor/actuator backend
│   ├── ml-hw-mqtt       # MQTT IoT backend
│   ├── ml-p2p           # Peer-to-peer communication
│   ├── ml-memphis       # Memphis chains integration
│   ├── ml-protocol      # ML ↔ Memphis Protocol translation
│   ├── ml-vm            # Stack-based bytecode VM
│   └── ml-cli           # CLI tool
├── examples/
│   ├── gate-control.ml
│   ├── temp-monitor.ml
│   └── distributed-heat.ml
└── SPEC.md
```

## GRAMATYKA (pełny język)

```
program    := stmt*
stmt       := expr | def | if | while | fn | return | import
def        := (def name expr)
if         := (if cond then else?)
while      := (while cond body)
fn         := (fn (args...) body)
return     := (return expr)
import     := (import "module")
expr       := atom | list | lambda | expr(args...)
atom       := number | string | bool | symbol
list       := (expr*)
lambda     := (fn (args...) body)
args       := symbol*
cond       := (op expr expr) | (and cond cond) | (or cond cond) | (not cond)
op         := + | - | * | / | == | != | > | < | >= | <=
```

## CO ROBIĆ TERAZ (szybki launch)

### Faza 1: ML Core (teraz)
- [x] lexer.rs (done)
- [x] parser.rs (done)
- [x] ast.rs (done)
- [x] machine.rs + MockMachine (done)
- [ ] Pełny parser (funkcje, zmienne, pętle)
- [ ] Type system (opcjonalny na początek)
- [ ] Tests (pisać ciągle)

### Faza 2: ML Lang (ambitne)
- [ ] Funkcje + lambdy
- [ ] Zmienne + scope (let/var)
- [ ] Pętle (while, for)
- [ ] Moduły + import
- [ ] Standard library (list, map, filter, reduce)

### Faza 3: Hardware Backends
- [ ] GPIO (Raspberry Pi via rppal lub sysfs)
- [ ] HTTP sensors (REST API)
- [ ] MQTT (pubsub IoT)

### Faza 4: Memphis Integration
- [ ] ML actions → journal.chain
- [ ] ML decisions → decisions.chain
- [ ] Memphis runtime spawns ML programs
- [ ] ML ↔ MP translation layer

### Faza 5: Distributed
- [ ] ML-P2P protocol
- [ ] Peer discovery
- [ ] Distributed execution
- [ ] Matrix transport (via Memphis chains)

## STATUS: Faza 1 w trakcie
