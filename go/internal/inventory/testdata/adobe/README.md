# Adobe license fixture

`cache.db-sample1.db` is a verbatim copy of the upstream GLPI Agent (Perl)
capture at `resources/generic/license/adobe/`, used to pin `parseAdobeLicenses`
(the port of `Tools/License.pm getAdobeLicensesWithoutSqlite`).

The pinned values are what the upstream **regex** path (`getAdobeLicensesWithoutSqlite`)
actually produces on this binary — verified by running the upstream Perl
directly. They intentionally differ from the idealized values in
`t/agent/tools/license.t`, which come from the SQLite-backed `getAdobeLicenses`
path: the regex path yields a looser FULLNAME (a trailing junk byte from the
greedy match) and only the non-letter-terminated component per product. We
mirror the regex path exactly, since that is the path the Windows inventory uses.

Source: GLPI Agent (Perl), GPL-2.0-or-later — same licence as this Go track.
