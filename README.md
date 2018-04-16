# Usage

```
rust-state 0.1.0
Erik Johnston

USAGE:
    rust-state <input> [postgres-connection]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <input>                  File containing the room events one per line
    <postgres-connection>    Postgres connection string
```

## Example Output

```
$ rust-state data/offtopic_events "postgres://localhost/synapse"
Reading took 2 seconds
Missing:
        $14741221720RLIbs:jki.re
Extremities:
        $1523491418553572iByVu:matrix.org
Roots:
        $143679705872209FGduw:matrix.org
Ordering took 0 seconds
1.59GB
██████████████████████████████████████████████████████████████████████████████████████████ 582221/582221
State calculation took 6 seconds
32284
Size: 55.48MB
Size: 3.61GB
5.65GB

First divergence: $1470095857182171kPpwH:matrix.org at 123676
 - (m.room.member, @shrike:matrix.org) $1465720357353298aBLxk:matrix.org
 + (m.room.member, @shrike:matrix.org) $1465559915118mMtpO:jki.re

Difference at extremity $1523491418553572iByVu:matrix.org
 - (m.room.member, @brathering:matrix.org) $14877107882171788ROtoY:matrix.org
 - (m.room.member, @ZerataX:matrix.org) $15227157793686189dLDaW:matrix.org
 - (m.room.member, @shrike:matrix.org) $1465720357353298aBLxk:matrix.org
 - (m.room.member, @freq301:matrix.org) $1515545891313658HpeBG:matrix.org
 - (m.room.aliases, thebeckmeyers.xyz) $15098433394186BHsAQ:thebeckmeyers.xyz
 - (m.room.aliases, disroot.org) $150636797216787ILMBs:disroot.org
 - (m.room.member, @lowee:matrix.org) $14892681051033134XEkrn:matrix.org
 - (m.room.member, @Rain:home.rdash.in) $14842341900jWshq:home.rdash.in
 - (m.room.aliases, matrix.eclabs.de) $151971982716oXWWP:matrix.eclabs.de
 + (m.room.member, @brathering:matrix.org) $14845125811738017doson:matrix.org
 + (m.room.member, @freq301:matrix.org) $15153092141198053kAoGE:matrix.org
 + (m.room.member, @Rain:home.rdash.in) $148544617122yjWAa:home.rdash.in
 + (m.room.aliases, matrix.eclabs.de) $149522286422970ogYkt:matrix.eclabs.de
 + (m.room.member, @shrike:matrix.org) $1465559915118mMtpO:jki.re
 + (m.room.member, @ZerataX:matrix.org) $15069532822038539ziiXD:matrix.org
 + (m.room.aliases, thebeckmeyers.xyz) $1497878877592XfwiY:thebeckmeyers.xyz
 + (m.room.aliases, disroot.org) $14955306921541EBirs:disroot.org
 + (m.room.member, @lowee:matrix.org) $15158727462635935quUAx:matrix.org
```
