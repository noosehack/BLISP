# BLISP Example Data

This directory contains test datasets for BLISP examples.

## Quickstart Data

Located in `quickstart/`:

### prices_10.csv
- 10 rows of synthetic price and volume data
- Format: DATE;PRICE;VOLUME
- Used by: `examples/quickstart/load_csv.blisp`, `examples/quickstart/rolling_window.blisp`

## Golden Test Data

Located in `gld_num_mini/`:

### RAW_FUT_PRC_100.csv
- First 100 rows of futures price data
- 127 columns of commodity futures prices
- Format: TIMESTAMP;[commodity columns]

### GC1C_100.csv
- First 100 rows of gold futures data
- Format: TIMESTAMP;GC1 Comdty

## Data Sources

The full datasets (RAW_FUT_PRC.csv, GC1C.csv) are maintained separately
and are not bundled with the repository due to size.

To generate mini datasets from full data:
```bash
head -101 RAW_FUT_PRC.csv > data/gld_num_mini/RAW_FUT_PRC_100.csv
head -101 GC1C.csv > data/gld_num_mini/GC1C_100.csv
```

## Format

All CSV files use semicolon (;) as the separator.
