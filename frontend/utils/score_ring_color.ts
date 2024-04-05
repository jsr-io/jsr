// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function getScoreBgColorClass(score: number): string {
  if (score >= 90) {
    return "score-ring-green";
  } else if (score >= 60) {
    return "score-ring-yellow";
  }
  return "score-ring-red";
}

export function getScoreTextColorClass(score: number): string {
  if (score >= 90) {
    return "score-text-green";
  } else if (score >= 60) {
    return "score-text-yellow";
  }
  return "score-text-red";
}
