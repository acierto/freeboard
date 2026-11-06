# Spec Delta

## Purpose

Let the user decide what to remove, either quickly through a size goal that fills the pile with Safe items, or precisely by area, group, item or part, with Keep and Ask me later decisions. Selection is read-only; removal happens only through a confirmed plan.

Source: `docs/reference/FREEBOARD_SPEC.md`, Building the pile, Item-level controls, Quick questions, Ordering, Milestone 4.

## ADDED Requirements

### Requirement: Pile state is owned by the backend
The pile SHALL be stored by the backend as a goal (bytes or All I can), areas (empty means everywhere), mode (auto or manual), added IDs, taken-out IDs, current item IDs and total. Every change SHALL emit `pile_changed`. The pile SHALL persist until the next cleanup or reset.

#### Scenario: Survives restart
- **WHEN** the user picks 100 GB, takes one crate out and restarts the app
- **THEN** the pile has the same goal, mode, items and total

### Requirement: Automatic filling
In auto mode, the pile SHALL start with items the user added, then add Safe items, largest first, until the goal is met. All I can SHALL add every Safe item in the chosen areas. Review and Risky items SHALL never be added automatically. Kept items, blocked items, changed items and items with no selected parts SHALL never be added.

#### Scenario: Fill to goal
- **WHEN** Safe items are 18.7, 14.8, 11.3, 9.2 and 6.2 GB and the goal is 50 GB
- **THEN** the pile holds the 18.7, 14.8, 11.3 and 9.2 GB items (54.0 GB)

#### Scenario: Review never auto-added
- **WHEN** the goal is All I can and a 41.2 GB Review item exists
- **THEN** the Review item is not in the pile unless the user added it

### Requirement: Pile rules
The pile SHALL follow these rules:
- Choosing 20, 50 or 100 GB fills to at least that amount.
- Choosing Where from limits automatic filling to those groups; Everywhere clears the limit.
- Adding an item or part keeps it in the pile even if it is not Safe.
- Taking a crate out removes it, switches to manual mode, and does not replace it.
- Keeping an item removes it from the pile and all automatic choices; Stop keeping makes it available again.
- Choosing a size again leaves manual mode and clears taken-out items.
- Changing Where from leaves manual mode but keeps taken-out items out.

#### Scenario: Take out does not refill
- **WHEN** the pile is filled to 50 GB and the user takes out the 14.8 GB crate
- **THEN** the total drops by 14.8 GB, mode is manual, and no other item is added

#### Scenario: Size again clears take-outs
- **WHEN** after taking a crate out the user chooses 50 GB again
- **THEN** mode is auto and the taken-out crate can be chosen again

#### Scenario: Where from keeps take-outs
- **WHEN** after taking a crate out the user changes Where from
- **THEN** mode is auto and the taken-out crate stays out

#### Scenario: Area limit
- **WHEN** Where from is Caches only
- **THEN** automatic filling adds only Safe items in the Caches group

### Requirement: Pile messages
In manual mode the home view SHALL say "You chose these by hand. Pick a size again to refill the pile." If the pile is empty, the main button SHALL be disabled with "The pile is empty. Pick a size to fill it again." If Safe items fall short of the goal, it SHALL show the remaining amount and point to the items below.

#### Scenario: Short of goal
- **WHEN** the goal is 100 GB and Safe items total 75.0 GB
- **THEN** the note says Safe crates stop at 75.0 GB and asks the user to add items below to find 25.0 GB more

### Requirement: Item and group controls
Everything I found and the Disk map SHALL offer per item a checkbox (add to or take out of the pile), Remove now (opens a confirmation for that item) and Keep (with Stop keeping). Each group and map area SHALL offer Remove all here and Add all to pile, which exclude blocked, kept and changed items and items that need individual confirmation. Items with parts SHALL offer Choose which.

#### Scenario: Group actions skip volumes
- **WHEN** the user chooses Add all to pile on the Docker group, which includes a volume needing confirmation
- **THEN** every eligible Docker item is added except the volume

#### Scenario: No parts selected
- **WHEN** the user deselects every part of an item
- **THEN** the item's checkbox and Remove now are disabled

### Requirement: Quick questions
Quick questions SHALL show one card at a time, largest first, drawn from Review and Risky items. It SHALL exclude blocked items, kept items, items in the pile, items set to Ask me later and items that changed during a cleanup. Each card SHALL show item, size, reason, location, last use, action, recovery and Choose which where available. Add to pile adds the item and moves on. Keep, don't ask again marks it Keep. Ask me later skips it until the finished state or the next scan. A Risky item needing confirmation SHALL require a tick before Add to pile works.

#### Scenario: Volume needs a tick
- **WHEN** the current card is a Docker volume
- **THEN** Add to pile is disabled until the user ticks "I understand this cannot be recovered."

#### Scenario: Bring back skipped
- **WHEN** all cards are answered and two were set to Ask me later
- **THEN** the finished state offers to ask about the 2 skipped items

### Requirement: Ordering
Quick questions and each group in Everything I found SHALL be sorted by size, largest first. Blocked items SHALL appear in a protected list with their reason, and SHALL NOT be selectable.

#### Scenario: Blocked worktree shown
- **WHEN** a dirty worktree is found
- **THEN** it appears in the protected list with its reason and has no checkbox or Remove now
