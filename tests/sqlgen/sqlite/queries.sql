-- Queries over users (`lang generate sql queries.sql` writes queries.lang).

-- name: create_user :one
-- Add a user; returns its id.
-- params: name: String, email: Option<String>
INSERT INTO users (name, email) VALUES (:name, :email) RETURNING id;

-- name: find_user :opt
-- params: id: Int
SELECT id, name, email, karma FROM users WHERE id = :id;

-- name: users_named :many
-- params: pattern: String
SELECT id, name FROM users WHERE name LIKE :pattern ORDER BY name;

-- name: add_karma :exec
-- params: id: Int, by: Int
UPDATE users SET karma = karma + :by WHERE id = :id;

-- name: karma_total :one
SELECT count(*) AS "users!: Int", sum(karma) AS "total: Int" FROM users;
