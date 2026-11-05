-- Fixed-price listings. A listed copy leaves owned_cards for the listing
-- (escrow): trades, the forge and selling cannot touch it, and it comes back
-- as it was when withdrawn.
CREATE TABLE listings (
    id BIGSERIAL PRIMARY KEY,
    seller_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    card TEXT NOT NULL,
    foil TEXT,
    obtained_at TIMESTAMPTZ NOT NULL,
    price BIGINT NOT NULL CHECK (price > 0),
    state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'sold', 'withdrawn')),
    buyer_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    fee BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ
);
CREATE INDEX listings_open ON listings (card, price) WHERE state = 'open';
CREATE INDEX listings_open_new ON listings (created_at) WHERE state = 'open';
CREATE INDEX listings_seller ON listings (seller_id, state);
CREATE INDEX listings_buyer ON listings (buyer_id, closed_at) WHERE state = 'sold';
