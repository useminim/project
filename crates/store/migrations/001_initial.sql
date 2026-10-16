-- Schéma initial. Chaque ligne garde l'objet complet en JSON (`body`, forme
-- de minim-core) ; les autres colonnes servent au tri, aux filtres et aux
-- contrôles. Toute la base est chiffrée par SQLCipher (D-31).

CREATE TABLE workflows (
    id TEXT PRIMARY KEY NOT NULL,
    priority INTEGER NOT NULL,
    version INTEGER NOT NULL,
    -- 1 pour un tombstone (workflow supprimé, gardé pour la synchronisation).
    deleted INTEGER NOT NULL DEFAULT 0,
    body TEXT NOT NULL
) STRICT;

CREATE INDEX workflows_evaluation_order ON workflows (deleted, priority, id);

CREATE TABLE execution_logs (
    id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    status TEXT NOT NULL,
    -- Date de début en nanosecondes depuis l'époque Unix (UTC), pour trier.
    started_at INTEGER NOT NULL,
    body TEXT NOT NULL
) STRICT;

CREATE INDEX execution_logs_recent ON execution_logs (started_at, id);
CREATE INDEX execution_logs_document ON execution_logs (document_id, started_at, id);
CREATE INDEX execution_logs_workflow ON execution_logs (workflow_id, started_at, id);
