DROP TABLE IF EXISTS Links;

CREATE TABLE IF NOT EXISTS Links (
  id SERIAL PRIMARY KEY,
  url TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  category TEXT NOT NULL DEFAULT 'News'
);
 
INSERT INTO
  Links(id, url, description, status, category)
VALUES
  (
    1,
    'https://dreadytofatroptsdj6io7l3xptbet6onoyno2yv7jicoxknyazubrad.onion/',
    'Dread',
    'active',
    'forum' 
  );
