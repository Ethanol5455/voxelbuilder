pipeline {
  agent { label 'alpine-agent' }
  
  // environment {
  //   WORKING_DIR = '/workspace/voxelbuilder'
  // }
  
  stages {
    // stage('Checkout') {
    //   steps {
    //     dir("/workspace") {
    //         sh 'git clone https://github.com/Ethanol5455/voxelbuilder'
    //     }
    //   }
    // }
    stage('Setup') {
      steps {
        sh 'apk add make cmake clang clang-libclang rust rustfmt cargo g++'
      }
    }
    // stage('Build') {
    //   steps {
    //     dir("${env.WORKING_DIR}") {
    //       sh 'cargo build --all'
    //     }
    //   }
    // }
    // stage('Test') {
    //   steps {
    //     dir("${env.WORKING_DIR}") {
    //       sh 'cargo test --all'
    //     }
    //   }
    // }
    // stage("Format") {
    //   steps {
    //     dir("${env.WORKING_DIR}") {
    //       sh 'cargo fmt --check --verbose'
    //     }
    //   }
    // }
  }
}