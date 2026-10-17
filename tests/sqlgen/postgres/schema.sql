DROP TABLE IF EXISTS posts;
DROP TABLE IF EXISTS authors;
DROP TYPE IF EXISTS status;
CREATE TYPE status AS ENUM ('draft', 'published');
CREATE TABLE authors (
    id bigserial PRIMARY KEY,
    name text NOT NULL,
    bio text
);
CREATE TABLE posts (
    id bigserial PRIMARY KEY,
    author bigint NOT NULL REFERENCES authors (id),
    title text NOT NULL,
    status status NOT NULL DEFAULT 'draft',
    tags text[] NOT NULL DEFAULT '{}',
    score numeric(6, 2),
    meta jsonb
);
