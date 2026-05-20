-- Create default user for auto-authentication
INSERT INTO users (id, email, display_name, created_at, updated_at)
VALUES (
    '00000000-0000-0000-0000-000000000001'::uuid,
    'default@lumos.local',
    'Default User',
    now(),
    now()
)
ON CONFLICT (id) DO NOTHING;
