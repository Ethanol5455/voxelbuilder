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
    stages {
        stage('Setup') {
          steps {
            sh 'pwd'
            sh 'ls -la'
            sh 'apk add make cmake clang clang-libclang rust rustfmt cargo g++'
          }
        }
    }
}
