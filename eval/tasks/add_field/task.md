# Add a field

The project in this directory keeps a list of notes (`src/notes.bdg`).
Give every note a `pinned: Bool` field (false when a note is created),
and add `pub fn pin(notes: List<Note>, id: Int) -> Result<List<Note>,
NoteError>` that returns the notes with note `id` pinned, or
`NoteError.missing(id)` when there is no such note. Keep the existing
functions working. `badger check` and `badger test` must pass.
