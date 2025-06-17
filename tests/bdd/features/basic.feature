Feature: Basic

  Background:
    Given a 2-worker node kind kubernetes cluster
    And the mayastor helm chart is installed
    And all io-engine nodes shall be listed by kubectl-mayastor

  Scenario: Creating a DiskPool on all nodes
    When a DiskPool CR is created on all nodes
    Then eventually the diskpool CRs shall be created and Online
    And the diskpools shall be listed by kubectl-mayastor as Online

  Scenario Outline: Creating a PVC
    Given a DiskPool CR is created on all nodes
    When a PVC with <repl> replicas is created with Immediate
    Then eventually it will become bound
    Examples:
      | repl |
      |   1  |
      |   2  |
