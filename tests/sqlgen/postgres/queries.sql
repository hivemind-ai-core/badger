-- Typed queries over authors and posts (`lang generate sql --database ...`).

-- name: create_author :one
INSERT INTO authors (name, bio) VALUES (:name, :bio?) RETURNING id;

-- name: create_post :one
INSERT INTO posts (author, title, tags, meta) VALUES (:author, :title, :tags, :meta) RETURNING id, status;

-- name: publish :exec
UPDATE posts SET status = 'published', score = :score WHERE id = :id;

-- name: post :opt
-- One post with its author.
SELECT p.id, p.title, p.status, p.tags, p.score, p.meta, a.name AS author, a.bio
FROM posts p JOIN authors a ON a.id = p.author
WHERE p.id = :id;

-- name: posts_tagged :many
SELECT id, title FROM posts WHERE :tag = ANY (tags) ORDER BY id;

-- name: author_stats :many
SELECT a.name, count(p.id) AS "posts!", avg(p.score)::float8 AS "average"
FROM authors a LEFT JOIN posts p ON p.author = a.id
GROUP BY a.name ORDER BY a.name;
