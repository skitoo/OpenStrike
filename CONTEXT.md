# OpenStrike

A reimplementation of the Sudden Strike engine that reproduces the original game's rules from an original copy's data.
The terms below follow the original game, and the manual's French names are given where they help.

## Players and sides

**Player**:
One of a map's player slots, each owning units. In single player, player 0 is the only human.
_Avoid_: Faction, owner

**Side**:
The camp a map gives each player (0 to 3); it decides who is friendly and who is hostile.
_Avoid_: Team (a multiplayer notion, numbered 1 to 4), nation

**Bloc**:
A group of mutually friendly sides that wins or loses together: sides 0 and 2 form one bloc, side 1 the other.
_Avoid_: Alliance, coalition

**Neutral side**:
Side 3, neither friendly nor hostile to any other side; it belongs to no bloc.

## Units

**Unit class**:
The kind of object that drives a unit's behaviour, as listed in the game's `UNITS` file (`tank`, `samo`, `apc`, `guner`…).
_Avoid_: Category, family

**Unit type**:
One named model of a unit class, with its own description (`t-34`, `tiger`).
_Avoid_: Model, kind

**Tank**:
A unit of the `tank` class: a vehicle whose turret aims independently of its hull. Assault guns and tank destroyers
without a turret are the separate `samo` class.

## Orders and behaviour

**Order**:
An instruction the player gives selected units, from the orders grid, a key or a right click (AVANCER, ATTAQUER,
ASSAUT, PIVOTER, HALTE).
_Avoid_: Command (the simulation's input), action

**Armed order**:
An order chosen in the grid that waits for a right click on its target before it is given.

**Automatic behaviour**:
What a unit does on its own when no order directs it, such as firing at an enemy that comes into sight.
_Avoid_: AI (reserved for computer players)

**Hold position**:
The toggle (TENIR LA POSITION, R) that keeps units from moving on their own to attack.

**Cease fire**:
The toggle (SUSPENDRE LE FEU, T) that keeps units from firing on their own.

## Milestones

**Tank skirmish**:
The first playable milestone: tanks of two blocs fight with the original rules until one bloc has no tanks left.
