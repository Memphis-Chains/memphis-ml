# ML Language Specification v1.0

**Memphis Language — Hardware Control + Distributed Agent Language**

---

## 1. Overview

ML is a Lisp-like (S-expression) language for:
- Hardware control (GPIO, sensors, actuators)
- Distributed computing (peer-to-peer agent communication)
- Integration with Memphis runtime (chains, vault, tools)

ML is:
- **Dynamic** — no static typing required (types inferred at runtime)
- **Functional** — functions are first-class values, no side effects by default
- **Imperative available** — when you need mutation (`set!`, `while`)
- **Distributed** — programs can span multiple machines
- **Audit-friendly** — every action is logged to Memphis chains

---

## 2. Syntax

### 2.1 S-Expressions

All ML code is written as S-expressions:

```ml
(argument1 argument2 ...)
```

### 2.2 Data Types

| Type       | Examples                              |
|------------|---------------------------------------|
| Numbers    | `42`, `-3.14`, `1e10`                 |
| Strings    | `"hello world"`                       |
| Booleans   | `true`, `false`                       |
| Symbols    | `my-variable`, `gate-1`, `temp.sensor` |
| Lists      | `[1 2 3]`                             |
| Functions  | `(fn (x) (+ x 1))`                    |
| Nil        | `nil`                                 |

### 2.3 Keywords (Special Forms)

- Control flow: `if`, `while`, `let`, `set!`, `fn`, `return`, `begin`
- Logic: `and`, `or`, `not`
- Hardware: `gate`, `read`, `actuate`
- Distributed: `spawn`, `send`, `receive`
- Utilities: `log`, `assert`, `import`, `try`, `throw`, `wait`

### 2.4 Operators

| Category      | Operators                          |
|---------------|------------------------------------|
| Arithmetic    | `+`, `-`, `*`, `/`, `%`            |
| Comparison    | `==`, `!=`, `<`, `>`, `<=`, `>=`   |
| Logical       | `and`, `or`, `not`                 |

---

## 3. Special Forms

### `let` — Bind a name to a value

```ml
(let name value body)
(let x 10 (+ x 5))  ; => 15
```

### `set!` — Mutate a bound variable

```ml
(set x 20)
```

### `if` — Conditional

```ml
(if condition then-expr else-expr?)
(if (> x 10) "big" "small")
```

### `while` — Loop

```ml
(while condition body)
(while (< i 10)
    (set i (+ i 1))
    (log i))
```

### `fn` — Define a function

```ml
(fn (arg1 arg2 ...) body)
(fn (x y) (* x y))
(fn (greet name) (str-concat greet " " name))
```

Functions are first-class values — they can be passed, returned, and stored.

### `return` — Early exit from a function

```ml
(fn (find-first pred list)
    (if (pred (head list))
        (return (head list)))
    (find-first pred (tail list)))
```

### `begin` — Sequence of expressions

```ml
(begin expr1 expr2 ... exprN)
(begin
    (set! x (+ x 1))
    (log "done")
    x)
```

---

## 4. Standard Library

### List Operations

| Function            | Description                          |
|---------------------|--------------------------------------|
| `(list ...)`        | Create a new list                    |
| `(head list)`       | First element                        |
| `(tail list)`       | Rest of the list                     |
| `(cons elem list)`  | Prepend element                      |
| `(length list)`     | Number of elements                   |
| `(map fn list)`     | Apply function to each element       |
| `(filter pred list)`| Keep elements where predicate is true |
| `(reduce fn init list)` | Fold left through the list      |

```ml
(list 1 2 3)                  ; => [1 2 3]
(head [a b c])                ; => a
(tail [a b c])                ; => [b c]
(cons 0 [1 2 3])              ; => [0 1 2 3]
(length [1 2 3])              ; => 3
(map (fn (x) (* x 2)) [1 2 3]) ; => [2 4 6]
(filter (fn (x) (> x 1)) [0 1 2]) ; => [1 2]
(reduce + 0 [1 2 3 4])        ; => 10
```

### String Operations

| Function              | Description                     |
|-----------------------|---------------------------------|
| `(str-length s)`      | Length of string                |
| `(str-concat s1 s2)`  | Concatenate two strings         |
| `(str-slice s start end)` | Substring                 |
| `(str-format fmt ...)`| Format string (like sprintf)     |

```ml
(str-length "hello")           ; => 5
(str-concat "hello" " world")  ; => "hello world"
(str-slice "hello" 1 4)        ; => "ell"
(str-format "temp is %" 22.5)  ; => "temp is 22.5"
```

### Math

| Function       | Description                     |
|----------------|---------------------------------|
| `(abs n)`      | Absolute value                  |
| `(floor n)`    | Round down                      |
| `(ceil n)`     | Round up                        |
| `(round n)`    | Round to nearest                |
| `(sqrt n)`     | Square root                     |
| `(pow n exp)`  | Power                           |
| `(random max)` | Random integer from 0 to max-1  |

```ml
(abs -5)       ; => 5
(floor 3.7)   ; => 3
(ceil 3.2)    ; => 4
(round 3.5)   ; => 4
(sqrt 16)     ; => 4
(pow 2 8)     ; => 256
(random 100)  ; => 42 (example)
```

### I/O

| Function      | Description                           |
|---------------|---------------------------------------|
| `(log ...)`   | Print values to stdout                |
| `(print ...)` | Print with newline                    |
| `(input prompt)` | Read a line from stdin            |

```ml
(log "Temperature:" temp)
(print "Done!")
(let name (input "Enter name: "))
```

---

## 5. Hardware Primitives

### Gates (Binary on/off devices)

```ml
(gate "garage" on)     ; open gate
(gate "garage" off)    ; close gate
(gate "garage" toggle) ; invert current state
```

### Sensors (Read-only values)

```ml
(read "temp.living_room")  ; => 22.5
(read "humidity.kitchen")  ; => 65.0
(read "motion.entry")      ; => true
(read "door.front")         ; => false
```

Sensor values are typed — numbers, booleans, or strings depending on the device.

### Actuators (Continuous control)

```ml
(actuate "dimmer.lamp" 0.5)   ; 50% power (0.0–1.0)
(actuate "servo.arm" 90)      ; 90 degrees
(actuate "heater.bath" 0.8)  ; 80% heat
```

---

## 6. Distributed Primitives

### Spawn — Run a program on another machine

```ml
(spawn "machine-2" program)
(spawn "pi-2" (fn (loop)
    (gate "garage" toggle)
    (wait 1000)
    (loop)))
```

### Send / Receive — Inter-machine messaging

```ml
(send "machine-2" message)    ; send to peer (returns acknowledgment)
(receive timeout?)            ; wait for message (timeout in ms, optional)
(let msg (receive 5000))      ; wait up to 5 seconds
```

Messages are ML values — numbers, strings, lists, or records.

### Peer Discovery

```ml
(discover)  ; => ["machine-2" "pi-1" "pi-2"]
```

Returns a list of currently available peer machine names.

---

## 7. Memphis Integration

Every ML action is automatically:

1. **Logged** to `journal.chain` — the raw event log
2. **Decided** on `decisions.chain` — the reason/context for actions
3. **Signed** with machine identity — cryptographic proof of origin

```ml
(memphis.log "Opening garage")                    ; raw event log
(memphis.journal "temperature_reading" {temp: 22.5}) ; structured journal
(memphis.decide "Opening garage because temp > 40") ; decision chain
```

### `wait` — Pause execution

```ml
(wait 5000)  ; sleep for 5000 milliseconds
```

---

## 8. Examples

### Gate + Temperature

```ml
(let temp (read "temp.server"))
(if (> temp 40)
    (begin
        (gate "fan" on)
        (log "Fan activated due to high temp"))
    (gate "fan" off))
```

### Function + Loop

```ml
(fn (check-temp threshold)
    (let t (read temp.sensor))
    (> t threshold))

(while true
    (if (check-temp 30)
        (gate "alarm" on))
    (wait 5000))
```

### Distributed Coordination

```ml
(spawn "pi-2"
    (fn (loop)
        (gate "garage" toggle)
        (wait 1000)
        (loop)))
(loop)
```

### Record and Filter

```ml
(let readings
    (list
        {sensor: "temp.living" value: 22.5}
        {sensor: "temp.bedroom" value: 19.0}
        {sensor: "temp.kitchen" value: 24.1}))

(let hot-rooms
    (filter (fn (r) (> r.value 23))
        readings))

(map (fn (r) (log r.sensor)) hot-rooms)
```

### Error Handling

```ml
(try
    (let temp (read "temp.unknown"))
    (if (> temp 30)
        (gate "fan" on))
(catch err
    (log "Sensor unavailable:" err)))
```

---

## 9. Grammar (EBNF)

```
program     := stmt*
stmt        := expr | def | import
def         := (let pattern expr)
expr        := atom | list | lambda | expr-tail
list        := "(" expr-tail ")"
expr-tail   := expr* | special-form
atom        := number | string | boolean | symbol
symbol      := [a-zA-Z_][a-zA-Z0-9_.-]*
lambda      := (fn (args?) body)
args        := symbol*
special-form:= if | while | let | set! | fn | return | begin
             | gate | read | actuate
             | spawn | send | receive | discover
             | log | print | input | import
             | try | throw | assert | wait
             | memphis.log | memphis.journal | memphis.decide
```

---

## 10. Error Handling

### try / catch

```ml
(try expr (catch error-var body))
```

Executes `expr`. If an error is thrown, binds it to `error-var` and evaluates `body`.

```ml
(try
    (set! result (read "temp.sensor"))
(catch err
    (set! result nil)
    (log "Failed:" err)))
```

### throw

```ml
(throw "error message")
(throw {code: "SENSOR_OFFLINE" msg: "temp.sensor unreachable"})
```

Throws an error value (string or record) that can be caught.

### assert

```ml
(assert condition "failure message")
```

If condition is `false`, throws the provided error message.

---

## 11. Safety

- **No arbitrary memory access** — ML cannot read/write raw memory addresses
- **Hardware sandboxing** — gate/read/actuate operations are limited to the device list registered with Memphis
- **Peer allowlist** — network access is restricted to declared peers in the Memphis cluster
- **Infinite loop detection** — loops are instrumented; after N iterations (configurable, default 1,000,000), execution is halted and an error is raised
- **Typed messages** — inter-machine messages are validated; unknown types are rejected
- **Chain integrity** — every action is cryptographically signed; tampering with journal or decisions chain is detectable

---

## 12. Implementation Notes

| Component   | Technology           | Location        |
|-------------|----------------------|-----------------|
| Parser      | Recursive descent    | `ml-core/`      |
| VM          | Stack-based bytecode | `ml-vm/`        |
| GPIO backend| sysfs / libgpiod     | `ml-hal-gpio/`  |
| HTTP backend| REST calls           | `ml-hal-http/`  |
| MQTT backend| MQTT pub/sub         | `ml-hal-mqtt/`  |

### Bytecode Instructions (Overview)

The VM uses a stack-based instruction set. Key instruction groups:

- **Stack**: `push`, `pop`, `dup`, `swap`
- **Control**: `jump`, `jump-if`, `call`, `ret`
- **Arithmetic**: `add`, `sub`, `mul`, `div`, `mod`
- **Comparison**: `eq`, `neq`, `lt`, `gt`, `lte`, `gte`
- ** locals**: `load`, `store`
- **Hardware**: `gate-on`, `gate-off`, `gate-toggle`, `sensor-read`, `actuate`
- **Distributed**: `spawn`, `send`, `recv`

---

*ML Language Specification v1.0 — Memphis*
