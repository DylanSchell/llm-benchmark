# Benchmark Scoring System

## Overview

The benchmark now includes a comprehensive scoring mechanism that makes it easy to sort and compare results by "best" performance. The scoring system balances three critical dimensions: correctness, speed, and token efficiency.

## Scoring Formula

### Composite Score Calculation

```
Composite Score = 0.5 × Success Rate + 0.3 × Speed Score + 0.2 × Token Score
```

### Component Breakdown

#### 1. Success Rate (50% weight)
- **Formula**: `successful_exercises / total_exercises`
- **Range**: 0.0 to 1.0 (0% to 100%)
- **Importance**: Highest weight because incorrect solutions have no value

#### 2. Speed Score (30% weight)
- **Formula**: `(max_time - actual_time) / (max_time - min_time)`
- **Range**: 0.0 to 1.0 (slower to faster)
- **Normalization**: Calculated within the filtered dataset
- **Condition**: Only calculated for successful runs

#### 3. Token Score (20% weight)
- **Formula**: `(max_tokens - actual_tokens) / (max_tokens - min_tokens)`
- **Range**: 0.0 to 1.0 (more tokens to fewer tokens)
- **Normalization**: Calculated within the filtered dataset
- **Condition**: Only calculated for successful runs

## Important Rules

### Success Requirement
- **If success_rate = 0%, composite_score = 0%** regardless of speed or token efficiency
- This ensures that only correct solutions are considered "good"

### Normalization Bounds
- Speed and token scores are normalized against the **current filtered dataset**
- Min/max values are calculated from **successful runs only**
- Results change dynamically based on filters applied

### Per-Language Scoring
- Scores should be compared within the same language category
- Different languages have different complexity levels and baseline performance

## Usage

### Web Dashboard

Visit `/scoring` to access the scoring dashboard:

1. **Model Rankings**: See all models ranked by average composite score
2. **Individual Results**: View detailed breakdown of each exercise result
3. **Filters**: Filter by language, agent, or quick bench only
4. **Sorting**: Click any column header to sort results

### API Endpoints

#### Get Scored Results
```bash
GET /api/scored-results?language=java&agent=pi
```

Response includes:
- `composite_score`: Overall weighted score (0-1)
- `success_rate`: Correctness metric (0-1)
- `speed_score`: Speed efficiency (0-1)
- `token_score`: Token efficiency (0-1)
- All other result metadata

#### Get Model Scores
```bash
GET /api/model-scores?language=javascript&quick=true
```

Returns aggregated model performance:
- Average scores across all exercises
- Total number of runs
- Sorted by composite score descending

### Example Calculation

For a run with:
- 200/225 exercises passed (88.9% success)
- 120 seconds duration (range: 60-300s in dataset)
- 50K tokens used (range: 30K-100K in dataset)

```
success_rate = 200/225 = 0.889
speed_score = (300 - 120) / (300 - 60) = 0.75
token_score = (100K - 50K) / (100K - 30K) = 0.714

composite_score = 0.5(0.889) + 0.3(0.75) + 0.2(0.714)
                = 0.445 + 0.225 + 0.143
                = 0.813 (or 81.3%)
```

## Interpretation

### Score Ranges

- **90%+**: Excellent - High correctness with good efficiency
- **80-89%**: Very Good - Strong performance across all dimensions
- **70-79%**: Good - Solid results with room for improvement
- **60-69%**: Fair - Some issues in one or more dimensions
- **Below 60%**: Needs Improvement - Significant gaps in correctness, speed, or efficiency

### Trade-offs

The weighting reflects these priorities:

1. **Correctness is paramount** (50%) - A fast, cheap solution that fails is worthless
2. **Speed matters** (30%) - Time is a practical constraint in real-world usage
3. **Token efficiency** (20%) - Important for cost optimization but secondary to quality

## Advanced Features

### Filtering Impact

When you apply filters, scores are recalculated based on the filtered subset:

- Filter by language → normalization bounds change to that language's performance
- Filter by agent → only compare results from that agent
- Quick bench only → faster results with fewer exercises

### Sorting Options

The dashboard supports sorting by:
- Composite Score (default)
- Success Rate
- Speed Score
- Token Score
- Duration
- Token Count

### Visual Indicators

- **Green bars**: High performance in each dimension
- **Color gradients**: Visual comparison across models
- **Rank positions**: Clear ordering from best to worst

## Future Enhancements

Potential improvements to consider:

1. **Geometric mean scoring**: Penalize imbalanced performance more heavily
2. **Percentile-based normalization**: Use 10th/90th percentiles instead of min/max to reduce outlier impact
3. **Time-decay weighting**: More recent results weighted higher
4. **Language-specific baselines**: Pre-computed benchmarks per language
5. **Confidence intervals**: Show statistical significance for model comparisons

## Technical Implementation

### Data Flow

1. Results are loaded from cache into `IndividualResult` objects
2. `calculate_scores()` computes all metrics for filtered results
3. Normalization bounds are calculated from successful runs
4. Each result gets individual scores and composite score
5. Results are sorted by composite score (descending)
6. Dashboard renders with visualizations and filtering

### Key Files

- `benchmark-web/src/services/result_service.rs`: Scoring logic
- `benchmark-web/src/routes/scoring.rs`: Route handlers
- `benchmark-web/templates/scoring.tera`: Dashboard UI
- `benchmark-web/src/services/benchmark_service.rs`: Service wrappers

## Troubleshooting

### No Scores Displayed

Possible causes:
- No successful runs in the filtered dataset
- All runs have 0% success rate (all scores will be 0)
- Filter combination returns no results

Solution: Clear filters or run some successful benchmarks first.

### Unexpected Score Rankings

Check:
- Are you comparing within the same language?
- What's the date range of results being compared?
- Are there outliers affecting min/max bounds?

Solution: Use more specific filters to create fair comparisons.

## Conclusion

The scoring system provides an objective, multi-dimensional way to evaluate and compare benchmark results. By balancing correctness, speed, and efficiency, it helps identify truly optimal solutions rather than those that excel in just one area.
