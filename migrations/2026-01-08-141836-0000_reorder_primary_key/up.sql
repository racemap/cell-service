-- Reorder primary key to optimize queries without radio filter
-- New order: (mcc, net, area, cell, radio) instead of (radio, mcc, net, area, cell)

ALTER TABLE cells DROP PRIMARY KEY,
    ADD PRIMARY KEY (mcc, net, area, cell, radio);
