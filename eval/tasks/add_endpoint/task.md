# Add an endpoint

`src/books.bdg` offers a `find` operation (an `app.Operation`) and
`src/web.bdg` binds `list` to `GET /books`. Bind `find` to
`GET /books/:id` with `endpoint.bind`: the path parameter `id` is the
operation's input, a found book is a 200 reply, and `BookError.missing`
is a 404 with the code `#books/missing`. Keep `GET /books` working.
