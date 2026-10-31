# Point smart orders

The selected client sends `smart` (`0xd0003`) for a ground click. The order registry
already resolves that name. `IssuePointOrder`, `IssuePointOrderById`,
`IssuePointOrderLoc`, and `IssuePointOrderByIdLoc` of that id now use the same
ground rules as a right click (`resolve_smart_order`): a unit that trains or has
rally gets `setrally`, a unit that can move gets the admitted move order, and a
unit that can do neither returns false. The concrete order is issued through the
same point native, so the queue stores `smart` and dispatches it when the unit
next has no order.

Target `smart` is unchanged and still means item pickup. An unknown order name
still fails instead of being treated as `smart`. Group point orders call the
per-unit activity consumer directly and do not take this path.

Not established: native queue length, and whether a scripted `smart` point order
differs from the resolved move or rally order in current-order reporting. This
host reports the concrete order, which is what the resolved right click already
reports.
