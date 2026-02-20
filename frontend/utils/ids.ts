// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export const validateScopeName = (name: string) => {
  if (name.length > 100) {
    return "Name must be less than 100 characters.";
  }
  if (name.length < 3) {
    return "Name must be at least 3 characters.";
  }
  if (!/^[a-z0-9-.]+$/.test(name)) {
    return "Name can only contain lowercase letters, numbers, dashes, and dots.";
  }
  if (name.startsWith(".") || name.endsWith(".")) {
    return "Name must not start or end with a dot.";
  }
  if (name.includes("..")) {
    return "Name must not contain consecutive dots.";
  }
  return null;
};

export const validateScopeDescription = (description: string) => {
  if (description.length > 200) {
    return "Description must be less than 200 characters.";
  }

  if (description !== "" && description.length < 5) {
    return "Description must be at least 5 characters. If you don't want to add a description, please leave it blank.";
  }

  return null;
};

export const validatePackageName = (name: string) => {
  if (name.startsWith("@")) {
    return "Enter only the package name, do not include the scope.";
  }
  if (name.length > 58) {
    return "Package name cannot be longer than 58 characters.";
  }
  if (!/^[a-z0-9\-]+$/.test(name)) {
    return "Package name can only contain lowercase letters, numbers, and hyphens.";
  }
  if (/^-/.test(name)) {
    return "Package name must start with a letter or number.";
  }

  return null;
};
