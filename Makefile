# Release helpers. Building and testing go through cargo directly (see AGENTS.md).
#
#   make bump            0.7.0 -> 0.7.1
#   make bump minor      0.7.0 -> 0.8.0
#   make bump major      0.7.0 -> 1.0.0
#
# Bumps the version, commits it, and says how to publish (git push).

.PHONY: bump patch minor major

# The word after `bump` (`make bump minor`); patch when there is none.
BUMP_PART := $(or $(word 2,$(MAKECMDGOALS)),patch)

bump:
	@bash scripts/bump-version.sh $(BUMP_PART)

# Let `patch`, `minor` and `major` ride along as goals without doing anything.
patch minor major:
	@:
