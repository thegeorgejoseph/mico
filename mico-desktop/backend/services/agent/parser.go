package agent

import (
	"encoding/json"
	"errors"
	"strings"
)

func parseToolCall(raw string) (ToolCall, error) {
	trimmed := strings.TrimSpace(raw)
	if tagged := extractTaggedAction(trimmed); tagged != "" {
		trimmed = tagged
	}
	var call ToolCall
	if err := json.Unmarshal([]byte(trimmed), &call); err == nil && call.Tool != "" {
		return call, call.Validate()
	}
	for _, candidate := range extractJSONObjectCandidates(trimmed) {
		if err := json.Unmarshal([]byte(candidate), &call); err == nil && call.Tool != "" {
			return call, call.Validate()
		}
	}
	return ToolCall{}, errors.New("provider did not return a valid tool call")
}

func extractTaggedAction(raw string) string {
	start := strings.LastIndex(raw, "<MICO_ACTION>")
	end := strings.LastIndex(raw, "</MICO_ACTION>")
	if start == -1 || end == -1 || end <= start {
		return ""
	}
	start += len("<MICO_ACTION>")
	return strings.TrimSpace(raw[start:end])
}

func extractJSONObjectCandidates(raw string) []string {
	candidates := make([]string, 0, 4)
	start := -1
	depth := 0
	inString := false
	escaped := false
	for index, r := range raw {
		if escaped {
			escaped = false
			continue
		}
		if r == '\\' && inString {
			escaped = true
			continue
		}
		if r == '"' {
			inString = !inString
			continue
		}
		if inString {
			continue
		}
		switch r {
		case '{':
			if depth == 0 {
				start = index
			}
			depth += 1
		case '}':
			if depth == 0 {
				continue
			}
			depth -= 1
			if depth == 0 && start >= 0 {
				candidates = append(candidates, raw[start:index+1])
				start = -1
			}
		}
	}
	for left, right := 0, len(candidates)-1; left < right; left, right = left+1, right-1 {
		candidates[left], candidates[right] = candidates[right], candidates[left]
	}
	return candidates
}

func excerpt(raw string) string {
	trimmed := strings.Join(strings.Fields(strings.TrimSpace(raw)), " ")
	if trimmed == "" {
		return "empty response"
	}
	if len(trimmed) > 240 {
		return trimmed[:240] + "..."
	}
	return trimmed
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}
