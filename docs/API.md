# API Documentation

This document describes the REST API endpoints provided by the Claude Benchmark Runner web interface.

---

## Base URL

```
http://localhost:8080
```

---

## Authentication

Currently, no authentication is required for local development. For production deployments, consider adding:
- Basic Auth
- API Key authentication
- OAuth2 integration

---

## Endpoints Overview

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/results` | List all benchmark results |
| GET | `/api/results/{sessionId}` | Get specific session results |
| POST | `/api/benchmark/run` | Start a new benchmark run |
| GET | `/api/exercises` | List available exercises |
| GET | `/api/queue` | Get queue status |
| POST | `/api/queue/pause` | Pause queue processing |
| POST | `/api/queue/resume` | Resume queue processing |
| DELETE | `/api/sessions/{sessionId}` | Cancel a session |

---

## Results API

### List All Results

**GET** `/api/results`

Returns a list of all benchmark sessions with summary information.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `status` | string | Filter by status (RUNNING, COMPLETED, FAILED) |
| `limit` | int | Maximum results to return (default: 50) |
| `offset` | int | Pagination offset (default: 0) |

**Response:**
```json
{
  "sessions": [
    {
      "sessionId": "sess_abc123",
      "agent": "claude",
      "model": "sonnet",
      "languages": ["java", "python"],
      "status": "COMPLETED",
      "startTime": "2026-02-28T10:00:00Z",
      "endTime": "2026-02-28T10:45:00Z",
      "totalExercises": 20,
      "completedExercises": 20,
      "successRate": 0.95
    }
  ],
  "pagination": {
    "total": 100,
    "limit": 50,
    "offset": 0
  }
}
```

**Status Codes:**
- `200 OK` - Success
- `400 Bad Request` - Invalid query parameters

---

### Get Session Results

**GET** `/api/results/{sessionId}`

Returns detailed results for a specific benchmark session.

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `sessionId` | string | The session ID |

**Response:**
```json
{
  "sessionId": "sess_abc123",
  "agent": "claude",
  "model": "sonnet",
  "languages": ["java"],
  "status": "COMPLETED",
  "startTime": "2026-02-28T10:00:00Z",
  "endTime": "2026-02-28T10:45:00Z",
  "exercises": [
    {
      "name": "two-fer",
      "language": "java",
      "status": "SUCCESS",
      "duration": 45.2,
      "exitCode": 0,
      "output": "...\nBUILD SUCCESS\n...",
      "traceFile": "results/sess_abc123/trace_java_two-fer.jsonl"
    },
    {
      "name": "hello-world",
      "language": "java",
      "status": "FAILED",
      "duration": 30.1,
      "exitCode": 1,
      "errorMessage": "Test compilation failed",
      "output": "...",
      "traceFile": "results/sess_abc123/trace_java_hello-world.jsonl"
    }
  ],
  "summary": {
    "totalExercises": 20,
    "successfulExercises": 19,
    "failedExercises": 1,
    "successRate": 0.95,
    "averageDuration": 38.5
  }
}
```

**Status Codes:**
- `200 OK` - Success
- `404 Not Found` - Session not found

---

## Benchmark API

### Start Benchmark Run

**POST** `/api/benchmark/run`

Starts a new benchmark run with the specified configuration.

**Form Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `agent` | string | Yes | Agent type: "reference" or "claude" |
| `language` | string[] | Yes | Languages to benchmark (e.g., java, python) |
| `model` | string | No | Model name (for Claude agent) |
| `exercise` | string | No | Specific exercise name (omit for all) |

**Request Example:**
```bash
curl -X POST http://localhost:8080/api/benchmark/run \
  -F "agent=claude" \
  -F "language=java" \
  -F "language=python" \
  -F "model=sonnet"
```

**Response:**
```json
{
  "sessionId": "sess_abc123",
  "status": "started",
  "redirectUrl": "/benchmark/sess_abc123"
}
```

**Status Codes:**
- `200 OK` - Benchmark started successfully
- `400 Bad Request` - Invalid parameters (e.g., no language selected)
- `500 Internal Server Error` - Server error

---

### Start Multiple Benchmarks (Batch)

**POST** `/api/benchmark/run/batch`

Starts multiple benchmark runs in sequence.

**Request Body:**
```json
{
  "benchmarks": [
    {
      "agent": "claude",
      "languages": ["java"],
      "model": "sonnet"
    },
    {
      "agent": "reference",
      "languages": ["java"]
    }
  ]
}
```

**Response:**
```json
{
  "queueId": "queue_xyz789",
  "sessions": [
    {"sessionId": "sess_abc123", "position": 1},
    {"sessionId": "sess_def456", "position": 2}
  ]
}
```

---

## Exercises API

### List Available Exercises

**GET** `/api/exercises`

Returns all available exercises from the polyglot-benchmark repository.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `language` | string | Filter by language |
| `name` | string | Filter by exercise name (partial match) |

**Response:**
```json
{
  "exercises": [
    {
      "name": "two-fer",
      "languages": ["java", "python", "javascript", "go", "rust"],
      "category": "beginner",
      "difficulty": "easy"
    },
    {
      "name": "hello-world",
      "languages": ["java", "python", "javascript", "go", "rust", "cpp"],
      "category": "beginner",
      "difficulty": "easy"
    }
  ],
  "totalCount": 50
}
```

**Status Codes:**
- `200 OK` - Success
- `500 Internal Server Error` - Could not load exercises

---

## Queue API

### Get Queue Status

**GET** `/api/queue`

Returns the current state of the benchmark queue.

**Response:**
```json
{
  "status": "PROCESSING",
  "currentSession": {
    "sessionId": "sess_abc123",
    "agent": "claude",
    "progress": {
      "currentExercise": "two-fer",
      "totalExercises": 20,
      "completedExercises": 15
    }
  },
  "queue": [
    {
      "sessionId": "sess_def456",
      "agent": "reference",
      "position": 1,
      "estimatedWaitSeconds": 300
    },
    {
      "sessionId": "sess_ghi789",
      "agent": "claude",
      "position": 2,
      "estimatedWaitSeconds": 600
    }
  ],
  "statistics": {
    "totalQueued": 2,
    "processingTimeSeconds": 1800,
    "averageExerciseTimeSeconds": 45
  }
}
```

**Status Codes:**
- `200 OK` - Success

---

### Pause Queue Processing

**POST** `/api/queue/pause`

Pauses processing of the benchmark queue.

**Request Body (optional):**
```json
{
  "reason": "Maintenance window"
}
```

**Response:**
```json
{
  "status": "PAUSED",
  "message": "Queue processing paused"
}
```

**Status Codes:**
- `200 OK` - Successfully paused
- `409 Conflict` - Queue already paused

---

### Resume Queue Processing

**POST** `/api/queue/resume`

Resumes processing of the benchmark queue.

**Response:**
```json
{
  "status": "PROCESSING",
  "message": "Queue processing resumed"
}
```

**Status Codes:**
- `200 OK` - Successfully resumed
- `409 Conflict` - Queue not paused

---

## Sessions API

### Cancel Session

**DELETE** `/api/sessions/{sessionId}`

Cancels a running benchmark session.

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `sessionId` | string | The session ID to cancel |

**Response:**
```json
{
  "sessionId": "sess_abc123",
  "status": "CANCELLED",
  "message": "Session cancelled successfully"
}
```

**Status Codes:**
- `200 OK` - Successfully cancelled
- `404 Not Found` - Session not found
- `409 Conflict` - Session already completed or failed

---

### Get Session Status

**GET** `/api/sessions/{sessionId}/status`

Returns the current status of a session.

**Response:**
```json
{
  "sessionId": "sess_abc123",
  "status": "RUNNING",
  "progress": {
    "currentExercise": "two-fer",
    "totalExercises": 20,
    "completedExercises": 15,
    "percentage": 75
  },
  "startTime": "2026-02-28T10:00:00Z",
  "elapsedSeconds": 1800
}
```

---

## Server-Sent Events (SSE)

### Session Progress Stream

**GET** `/api/sse/{sessionId}`

Opens a server-sent event stream for real-time progress updates.

**Event Types:**

| Event | Data Format | Description |
|-------|-------------|-------------|
| `session_started` | See below | Session initialized |
| `exercise_started` | See below | Starting an exercise |
| `exercise_progress` | See below | Progress update during execution |
| `exercise_completed` | See below | Exercise finished |
| `session_completed` | See below | All exercises done |
| `error` | See below | Error occurred |

**Event Data Examples:**

```javascript
// session_started
{
  "sessionId": "sess_abc123",
  "agent": "claude",
  "model": "sonnet",
  "languages": ["java"],
  "totalExercises": 20
}

// exercise_started
{
  "exerciseName": "two-fer",
  "language": "java"
}

// exercise_progress
{
  "exerciseName": "two-fer",
  "language": "java",
  "output": "Running tests...\n",
  "timestamp": "2026-02-28T10:30:00Z"
}

// exercise_completed
{
  "exerciseName": "two-fer",
  "language": "java",
  "status": "SUCCESS",
  "duration": 45.2,
  "exitCode": 0
}

// session_completed
{
  "sessionId": "sess_abc123",
  "status": "COMPLETED",
  "successRate": 0.95,
  "totalDuration": 1800
}

// error
{
  "error": "Docker container failed to start",
  "sessionId": "sess_abc123"
}
```

**Usage Example:**
```javascript
const eventSource = new EventSource('/api/sse/sess_abc123');

eventSource.addEventListener('exercise_progress', (event) => {
  const data = JSON.parse(event.data);
  console.log(`[${data.language}] ${data.exerciseName}: ${data.output}`);
});

eventSource.addEventListener('session_completed', (event) => {
  const data = JSON.parse(event.data);
  console.log(`Session complete! Success rate: ${data.successRate}`);
  eventSource.close();
});
```

---

## Error Responses

### Standard Error Format

All error responses follow this format:

```json
{
  "error": "Error message",
  "code": "ERROR_CODE",
  "details": {
    "field": "Additional context"
  },
  "timestamp": "2026-02-28T10:30:00Z"
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `INVALID_REQUEST` | 400 | Invalid request parameters |
| `SESSION_NOT_FOUND` | 404 | Session does not exist |
| `EXERCISE_NOT_FOUND` | 404 | Exercise not found |
| `CONFLICT` | 409 | Operation conflicts with current state |
| `TIMEOUT` | 504 | Request timed out |
| `INTERNAL_ERROR` | 500 | Internal server error |

---

## Rate Limiting

For production deployments, consider implementing rate limiting:

```
100 requests per minute per IP address
```

Headers included in responses:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1677583200
```

---

## Versioning

API version is included in the URL path:

```
/api/v1/results
/api/v1/benchmark/run
```

Current version: `v1`

---

## SDK Clients

### Java Client Example

```java
BenchmarkClient client = new BenchmarkClient("http://localhost:8080");

// Start a benchmark
String sessionId = client.startBenchmark(
    "claude",
    new String[]{"java", "python"},
    "sonnet"
);

// Get results
BenchmarkResult result = client.getResult(sessionId);
System.out.println("Success rate: " + result.getSummary().getSuccessRate());
```

### Python Client Example

```python
from benchmark_client import BenchmarkClient

client = BenchmarkClient("http://localhost:8080")

# Start a benchmark
session_id = client.start_benchmark(
    agent="claude",
    languages=["java", "python"],
    model="sonnet"
)

# Get results
result = client.get_result(session_id)
print(f"Success rate: {result.summary.success_rate}")
```

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [Configuration Reference](CONFIGURATION.md)
- [Developer Guide](DEVELOPER.md)

---

**Version:** 1.0  
**Last Updated:** 2026-02-28
