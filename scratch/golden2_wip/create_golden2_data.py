#!/usr/bin/env python3
"""
Generate test data for GOLDEN_2: Irregular timestamps + high NA density

Stresses OBS semantics with:
- Calendar gaps (weekends, holidays)
- ~30% NA rate (clustered, not IID)
- Business day irregularities
"""

import csv
from datetime import datetime, timedelta
import random

def generate_irregular_dates(start_date, num_days, holiday_prob=0.05):
    """Generate dates with weekends and random holidays."""
    dates = []
    current = start_date

    while len(dates) < num_days:
        # Skip weekends (Saturday=5, Sunday=6)
        if current.weekday() < 5:
            # Random holiday (5% chance)
            if random.random() > holiday_prob:
                dates.append(current)
        current += timedelta(days=1)

    return dates

def generate_price_series(dates, base_price=100.0, volatility=0.02, na_rate=0.30):
    """Generate price series with clustered NAs."""
    prices = []
    current_price = base_price
    in_na_cluster = False
    na_cluster_remaining = 0

    for i, date in enumerate(dates):
        # Clustered NA logic
        if in_na_cluster:
            prices.append(None)
            na_cluster_remaining -= 1
            if na_cluster_remaining <= 0:
                in_na_cluster = False
        elif random.random() < na_rate / 3:  # Start NA cluster
            in_na_cluster = True
            na_cluster_remaining = random.randint(2, 5)  # Cluster size 2-5
            prices.append(None)
        else:
            # Valid price: random walk
            change = random.gauss(0, volatility)
            current_price *= (1 + change)
            prices.append(current_price)

    return prices

def main():
    random.seed(42)  # Deterministic

    # Generate 500 business days (~2 years) with gaps
    start = datetime(2020, 1, 1)
    dates = generate_irregular_dates(start, 500, holiday_prob=0.05)

    # Generate price series with ~30% NA (clustered)
    prices = generate_price_series(dates, base_price=100.0, na_rate=0.30)

    # Write CSV
    with open('GOLDEN_2_DATA.csv', 'w', newline='') as f:
        writer = csv.writer(f, delimiter=';')
        writer.writerow(['DATE', 'PRICE'])

        for date, price in zip(dates, prices):
            date_str = date.strftime('%Y%m%d')
            price_str = f'{price:.6f}' if price is not None else ''
            writer.writerow([date_str, price_str])

    # Stats
    na_count = sum(1 for p in prices if p is None)
    print(f"Generated {len(dates)} dates")
    print(f"NA count: {na_count} ({na_count/len(dates)*100:.1f}%)")
    print(f"First date: {dates[0].strftime('%Y-%m-%d')}")
    print(f"Last date: {dates[-1].strftime('%Y-%m-%d')}")
    print(f"Saved to: GOLDEN_2_DATA.csv")

if __name__ == '__main__':
    main()
