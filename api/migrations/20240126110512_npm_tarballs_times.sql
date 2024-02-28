ALTER TABLE npm_tarballs
ADD COLUMN created_at timestamp with time zone NOT NULL DEFAULT now();
ALTER TABLE npm_tarballs
ADD COLUMN updated_at timestamp with time zone NOT NULL DEFAULT now();