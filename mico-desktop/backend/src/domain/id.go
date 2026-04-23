package domain

import (
	"strings"

	"github.com/google/uuid"
)

func NewID(prefix string) string {
	id := strings.TrimSpace(prefix)
	if id == "" {
		return uuid.NewString()
	}
	return id + "_" + uuid.NewString()
}
