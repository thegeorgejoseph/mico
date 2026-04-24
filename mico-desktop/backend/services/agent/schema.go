package agent

import (
	"os"
)

func outputSchema() (string, error) {
	return toolOutputSchema, nil
}

func writeOutputSchema() (string, func(), error) {
	schemaJSON, err := outputSchema()
	if err != nil {
		return "", nil, err
	}
	file, err := os.CreateTemp("", "mico-agent-schema-*.json")
	if err != nil {
		return "", nil, err
	}
	if _, err := file.WriteString(schemaJSON); err != nil {
		file.Close()
		os.Remove(file.Name())
		return "", nil, err
	}
	if err := file.Close(); err != nil {
		os.Remove(file.Name())
		return "", nil, err
	}
	return file.Name(), func() { _ = os.Remove(file.Name()) }, nil
}

func createOutputFile(pattern string) (string, func(), error) {
	file, err := os.CreateTemp("", pattern)
	if err != nil {
		return "", nil, err
	}
	if err := file.Close(); err != nil {
		os.Remove(file.Name())
		return "", nil, err
	}
	return file.Name(), func() { _ = os.Remove(file.Name()) }, nil
}

const toolOutputSchema = `{
  "type": "object",
  "additionalProperties": false,
  "required": [
    "tool",
    "reason",
    "selectRepo",
    "selectWorktree",
    "selectSession",
    "listRepos",
    "listWorktrees",
    "listSessions"
  ],
  "properties": {
    "tool": {
      "type": "string",
      "enum": [
        "select_repo",
        "select_worktree",
        "select_session",
        "list_repos",
        "list_worktrees",
        "list_sessions"
      ]
    },
    "reason": {
      "type": "string",
      "minLength": 1
    },
    "selectRepo": {
      "type": ["object", "null"],
      "additionalProperties": false,
      "required": ["repoId"],
      "properties": {
        "repoId": { "type": "string", "minLength": 1 }
      }
    },
    "selectWorktree": {
      "type": ["object", "null"],
      "additionalProperties": false,
      "required": ["worktreeId"],
      "properties": {
        "worktreeId": { "type": "string", "minLength": 1 }
      }
    },
    "selectSession": {
      "type": ["object", "null"],
      "additionalProperties": false,
      "required": ["sessionId"],
      "properties": {
        "sessionId": { "type": "string", "minLength": 1 }
      }
    },
    "listRepos": {
      "type": ["object", "null"],
      "additionalProperties": false
    },
    "listWorktrees": {
      "type": ["object", "null"],
      "additionalProperties": false
    },
    "listSessions": {
      "type": ["object", "null"],
      "additionalProperties": false
    }
  }
}`
