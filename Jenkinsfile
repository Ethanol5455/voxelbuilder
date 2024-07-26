// Uses Declarative syntax to run commands inside a container.
pipeline {
    agent {
        kubernetes {
            yaml '''
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: shell
    image: alpine
    command:
    - sleep
    args:
    - infinity
    securityContext:
      runAsUser: 0
'''
            defaultContainer 'shell'
            retries 2
        }
    }
    triggers {
      githubPush()
    }
    stages {
        stage('Setup') {
          steps {
            sh 'apk add make cmake clang clang-libclang rust rustfmt cargo g++'
          }
        }
        stage ("Tests") {
          parallel {
            stage("Building") {
              stage('Build') {
                steps {
                  sh 'cargo build --all'
                }
              }
              stage('Test') {
                steps {
                  sh 'cargo test --all'
                }
              }
            }
            stage("Format") {
              steps {
                sh 'cargo fmt --check --verbose'
              }
            }
          }
        }       
    }
}
